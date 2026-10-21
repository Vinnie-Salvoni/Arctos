$path = "Cargo.toml"
$content = Get-Content $path
$newContent = @()
$bumped = $false

foreach ($line in $content) {
    # Only bump the first version matching the main package
    if (-not $bumped -and $line -match '^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"') {
        $major = [int]$matches[1]
        $minor = [int]$matches[2]
        $patch = [int]$matches[3]
        $patch++
        $newContent += "version = `"$major.$minor.$patch`""
        Write-Host "Successfully bumped version to $major.$minor.$patch"
        $bumped = $true
    } else {
        $newContent += $line
    }
}

Set-Content -Path $path -Value $newContent
