# vendor/council-of-high-intelligence

This directory is a **git submodule checkout**, not source we edit.

```
git submodule add https://github.com/0xNyk/council-of-high-intelligence.git \
  vendor/council-of-high-intelligence
git -C vendor/council-of-high-intelligence checkout v1.2.0
```

Then put the exact `git rev-parse HEAD` into `council-pin.json`.

Do not run `install.sh` here. Do not commit edits inside the submodule.
Updates go through `scripts/council-vendor-sync.sh`.
