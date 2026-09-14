$ErrorActionPreference = "Stop"

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$imageName = "svn-ai-e2e:local"
$dockerfile = Join-Path $repositoryRoot "tests\e2e\Dockerfile"

& docker build --file $dockerfile --tag $imageName $repositoryRoot
if ($LASTEXITCODE -ne 0) {
    throw "Failed to build the Claude/SVN validation image."
}

$sourceMount = "type=bind,source=$repositoryRoot,target=/workspace"
$targetMount = "type=volume,source=svn-ai-e2e-target,target=/workspace/target"
$registryMount = "type=volume,source=svn-ai-e2e-registry,target=/usr/local/cargo/registry"

& docker run --rm `
    --mount $sourceMount `
    --mount $targetMount `
    --mount $registryMount `
    --workdir /workspace `
    $imageName `
    bash tests/e2e/claude_hook_svn_round_trip.sh
if ($LASTEXITCODE -ne 0) {
    throw "Claude Code to SVN end-to-end validation failed."
}
