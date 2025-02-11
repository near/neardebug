build *BUILDFLAGS: # pass `--dev --no-opt` for debugging the glue itself
    wasm-pack build --target=web {{ BUILDFLAGS }}

ghpages: build
    mkdir -p _site
    cp -v *.html _site
    cp -v *.js _site
    cp -v *.css _site
    mv -v pkg _site
