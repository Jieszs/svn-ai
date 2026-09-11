#!/usr/bin/env bash
set -euo pipefail

test_root="$(mktemp -d)"
repository="$test_root/repository"
working_copy="$test_root/working-copy"
before_file="$test_root/before.txt"
ai_file="$test_root/ai-after.txt"
attribution_file="$test_root/attribution.json"
metrics_file="$test_root/metrics.json"
fingerprint_key="0707070707070707070707070707070707070707070707070707070707070707"

echo "[e2e] Subversion $(svn --version --quiet)"
svnadmin create "$repository"
svn checkout "file://$repository" "$working_copy" --quiet

mkdir "$working_copy/trunk"
printf 'legacy-line\n' > "$working_copy/trunk/code.txt"
svn add "$working_copy/trunk" --quiet
svn commit "$working_copy" -m 'baseline' --username zhengjie --no-auth-cache --quiet

cp "$working_copy/trunk/code.txt" "$before_file"
cp "$before_file" "$ai_file"
for line_number in $(seq -w 1 10); do
    printf 'ai-line-%s\n' "$line_number" >> "$ai_file"
done

repository_uuid="$(svnlook uuid "$repository")"
revision_author="$(svnlook author -r 1 "$repository")"
cargo run --quiet -p svn-ai-validate -- capture \
    --before "$before_file" \
    --after "$ai_file" \
    --repo-path trunk/code.txt \
    --repository-uuid "$repository_uuid" \
    --svn-username "$revision_author" \
    --base-revision 1 \
    --key-hex "$fingerprint_key" \
    --out "$attribution_file"

if grep -Fq 'ai-line-' "$attribution_file"; then
    echo '[e2e] attribution file leaked source code' >&2
    exit 1
fi

cp "$ai_file" "$working_copy/trunk/code.txt"
sed -i 's/ai-line-04/human-line-04/' "$working_copy/trunk/code.txt"
sed -i 's/ai-line-08/human-line-08/' "$working_copy/trunk/code.txt"
svn commit "$working_copy" -m 'AI lines with two human rewrites' --username zhengjie --no-auth-cache --quiet

cargo run --quiet -p svn-ai-validate -- verify \
    --svnlook svnlook \
    --repository "$repository" \
    --revision 2 \
    --attribution "$attribution_file" \
    --key-hex "$fingerprint_key" \
    --json | tee "$metrics_file"

jq -e '
    .svn_additions == 10 and
    .ai_additions == 8 and
    .non_ai_additions == 2 and
    .ambiguous_additions == 0
' "$metrics_file" >/dev/null

echo '[e2e] PASS: real SVN commit attributed 8 AI lines and 2 non-AI lines'

