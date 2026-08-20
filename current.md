Before adding more behavior, fix one important portability issue: generated source crates should not depend on the Core crate through an absolute checkout path such as:

lexicon-core = { path = "/workspaces/lexicon/lexicon-framework/core" }

That only works because the verification machine still has the Lexicon repository at that location. An external project would break after moving machines or removing that checkout.

The stable solution is a versioned dependency:

[dependencies]
lexicon-core = "=0.1.0"

If Core is not yet published, temporarily use a pinned Git tag:

lexicon-core = {
    git = "https://github.com/<owner>/<repo>",
    tag = "v0.1.0"
}

Core already is a crate; this does not require inventing another architectural component.

The next micro step is therefore:

1. Make generated manifests use a portable, version-pinned Core dependency.
2. Generate a project under /tmp.
3. Confirm its manifests contain no /workspaces/lexicon path.
4. Run cargo check on both generated crates.

After that, add the minimal ProcessData trait and make the generated processing crate implement it, matching the acquisition crate’s compile-time enforcement. Then the next user-facing feature will be the source build/add command that compiles both implementations into runtime executables.
