curl -o "$0/packages/uniprops_gen/UnicodeData.txt" https://www.unicode.org/Public/UCD/latest/ucd/UnicodeData.txt
if git diff --quiet; then
    echo "No changes detected."
    echo "changes=false" >> $GITHUB_OUTPUT
    return
else
    echo "Changes detected."
    echo "changes=true" >> $GITHUB_OUTPUT
fi
cargo install cargo-edit
cargo set-version --bump patch -p uniprops_gen -p dec

NEW_VERSION=$(cargo metadata --format-version 1 --no-deps | jq -r '.packages[0].version')
echo "New version is $NEW_VERSION"
echo "new_version=$NEW_VERSION" >> $GITHUB_OUTPUT

# vdigits="[0-9]+\.[0-9]+\.[0-9]+"
# sed -E -i "s/rlibphonenumber = \"$vdigits\"/rlibphonenumber = \"$NEW_VERSION\"/g" "$project_home/Readme.md"
# sed -E -i "s/Used metadata version: v$vdigits/Used metadata version: $tag_name/g" "$project_home/Readme.md"
