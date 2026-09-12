# Manual local deploy to Cloudflare Pages
# Requires: CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID environment variables set

Write-Host "Building site with taxus..." -ForegroundColor Cyan
taxus build

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed. Aborting deploy." -ForegroundColor Red
    exit 1
}

Write-Host "Deploying to Cloudflare Pages..." -ForegroundColor Cyan
# Pin the branch: without --branch, wrangler uses the current git branch,
# so running this off trunk silently creates a preview deployment instead
# of updating production.
npx wrangler pages deploy dist/ --project-name=crusty-metallian-net --branch=trunk

if ($LASTEXITCODE -eq 0) {
    Write-Host "Deploy complete!" -ForegroundColor Green
} else {
    Write-Host "Deploy failed." -ForegroundColor Red
}
