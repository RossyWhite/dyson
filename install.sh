#!/bin/sh

set -e

get_os() {
  uname -s | awk '{print tolower($0)}'
}

owner="RossyWhite"
repo="dyson"
exe_name="dyson"
downloadFolder="$(mktemp -d)"
executable_folder="/usr/local/bin"

echo "[1/4] Fetching latest version"
curl -sL "https://api.github.com/repos/$owner/$repo/releases/latest" \
  -H "Accept: application/vnd.github.v3+json" \
  -o "$downloadFolder"/output.json

asset_id=$(jq -r ".assets[] | select(.name | test(\"^.*$(get_os).*$\")) | .id" "$downloadFolder"/output.json)
file_name=$(jq -r ".assets[] | select(.name | test(\"^.*$(get_os).*$\")) | .name" "$downloadFolder"/output.json)

downloaded_file="${downloadFolder}/${file_name}"

echo "[2/4] Downloading ${file_name}"
rm -f "${downloaded_file}"
curl -fsSL -H "Accept: application/octet-stream" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  https://api.github.com/repos/"${owner}"/"${repo}"/releases/assets/"${asset_id}" \
  -o "${downloaded_file}"

echo "[3/4] Install ${exe_name} to the ${executable_folder}"
tar -xz -f "${downloaded_file}" -C ${executable_folder}
exe=${executable_folder}/${exe_name}
chmod +x "${exe}"

echo "[4/4] Set environment variables"
echo "${exe_name} was installed successfully to ${exe}"
if command -v "$exe_name" --help >/dev/null; then
  echo "Run '$exe_name --help' to get started"
else
  echo "Manually add the directory to your \$HOME/.bash_profile (or similar)"
  echo "  export PATH=${executable_folder}:\$PATH"
  echo "Run '$exe_name --help' to get started"
fi

exit 0
