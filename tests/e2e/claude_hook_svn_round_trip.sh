#!/usr/bin/env bash
set -euo pipefail

test_root="$(mktemp -d)"
repository="$test_root/repository"
working_copy="$test_root/working-copy"
client_home="$test_root/client-home"
settings_file="$test_root/claude/settings.json"
hook_input="$test_root/hook.json"
events_file="$test_root/events.json"
metrics_file="$test_root/metrics.json"
fingerprint_key="0707070707070707070707070707070707070707070707070707070707070707"
binary="/workspace/target/debug/svn-ai"

echo "[claude-svn-e2e] Subversion $(svn --version --quiet)"
cargo build --quiet -p svn-ai
svnadmin create "$repository"
svn checkout "file://$repository" "$working_copy" --quiet

mkdir "$working_copy/trunk"
printf 'legacy-line\n' > "$working_copy/trunk/code.txt"
svn add "$working_copy/trunk" --quiet
svn commit "$working_copy" -m 'baseline' --username zhengjie --no-auth-cache --quiet
revision_author="$(svnlook author -r 1 "$repository")"

"$binary" --home "$client_home" configure \
    --device-id device-1 \
    --svn-username "$revision_author" \
    --fingerprint-key "$fingerprint_key" \
    --svn svn
"$binary" --home "$client_home" install-hooks \
    --settings "$settings_file" \
    --executable "$binary" >/dev/null

jq -e '
    .hooks.PreToolUse[0].matcher == "Edit|Write" and
    .hooks.PostToolUse[0].matcher == "Edit|Write" and
    .hooks.PostToolUseFailure[0].matcher == "Edit|Write"
' "$settings_file" >/dev/null

jq -n \
    --arg cwd "$working_copy" \
    --arg file "$working_copy/trunk/code.txt" \
    '{
        session_id: "session-automatic-1",
        transcript_path: "/private/transcript.jsonl",
        cwd: $cwd,
        permission_mode: "default",
        hook_event_name: "PreToolUse",
        tool_name: "Edit",
        tool_input: {
            file_path: $file,
            old_string: "legacy-line",
            new_string: "AI generated content"
        },
        tool_use_id: "toolu_automatic_1"
    }' > "$hook_input"
"$binary" --home "$client_home" hook < "$hook_input"

for line_number in $(seq -w 1 10); do
    printf 'ai-line-%s\n' "$line_number" >> "$working_copy/trunk/code.txt"
done

jq '.hook_event_name = "PostToolUse" | .tool_response = {ok: true}' \
    "$hook_input" > "$hook_input.post"
"$binary" --home "$client_home" hook < "$hook_input.post"

"$binary" --home "$client_home" status --json \
    | jq -e '.pending_transactions == 0 and .attribution_events == 1' >/dev/null
"$binary" --home "$client_home" events --json | tee "$events_file" >/dev/null
if grep -Fq 'ai-line-' "$events_file" \
    || grep -Fq "$working_copy" "$events_file" \
    || grep -Fq 'session-automatic-1' "$events_file" \
    || grep -Fq 'transcript.jsonl' "$events_file"; then
    echo '[claude-svn-e2e] finalized attribution leaked private source or paths' >&2
    exit 1
fi

sed -i 's/ai-line-04/human-line-04/' "$working_copy/trunk/code.txt"
sed -i 's/ai-line-08/human-line-08/' "$working_copy/trunk/code.txt"
svn commit "$working_copy" -m 'Claude lines with two human rewrites' \
    --username zhengjie --no-auth-cache --quiet

"$binary" --home "$client_home" stats \
    --repository "$repository" \
    --revision 2 \
    --svnlook svnlook \
    --json | tee "$metrics_file"

jq -e '
    .svn_additions == 10 and
    .ai_additions == 8 and
    .non_ai_additions == 2 and
    .ambiguous_additions == 0
' "$metrics_file" >/dev/null

echo '[claude-svn-e2e] PASS: Claude hooks automatically attributed 8 AI lines and 2 non-AI lines'
