Overall, it aligns with the updated direction, but there are several corrections before calling the source-creation flow complete.

Corrections:

1. The verification did not test the public CLI path. It ran:

cargo run -p lexicon-framework -- source new example-source

The end-to-end test must run:

cargo run -p lexicon-cli -- source new <fresh-test-name>

Using an existing source only tests the duplicate-source guard, not successful generation or CLI-to-framework dispatch.

2. Source-name validation must exist inside the framework, not only the CLI, because the framework performs filesystem writes and can be invoked directly. At minimum, allow a form such as:

[a-z0-9]+(-[a-z0-9]+)*

and reject path separators, .., absolute paths, and unsafe platform names.

3. HttpAcquisition::run(&self) proves a Rust trait connection, but it is not yet meaningfully HTTP-specific. Currently, the word “HTTP” exists only in the trait name.
4. source new replaces --draft; it does not replace the source-building concept represented by --add. That operation must remain pending until deliberately redesigned.
5. The definitive specification must be updated to include source new, source.toml, discovery.md, and the protocol behavior.

The next micro step should be to make the HTTP interface structurally real by introducing a Core-owned context:

pub trait HttpAcquisition {
    fn acquire(
        &self,
        context: &mut HttpAcquisitionContext,
    ) -> Result<(), String>;
}

Then run_http_source should:

1. Construct HttpAcquisitionContext.
2. Pass it to implementation.acquire(&mut context).
3. Return the implementation result.

The generated source should implement that signature, even if the context initially has no methods. This establishes the correct boundary:

Core owns HTTP execution context
→ concrete source receives that context
→ later Core adds context.send(...)
→ context.send(...) creates request/response records

After that change, verify a fresh generated get-raw-data crate with cargo check. The following step can add the first real HttpAcquisitionContext::send behavior.

Yes. The current implementation can mechanically create lexicon-framework/sources/ using create_dir_all, but that assumes the user is modifying the Lexicon framework repository itself. That is not a complete external-user workflow.

There should be a project initialization command:

lexicon init <project-name>

It creates:

<project-name>/
├── lexicon.toml
└── sources/

For example:

lexicon init my-data-project
cd my-data-project
lexicon source new example-source

source new should then:

1. Search upward from the current directory for lexicon.toml.
2. Treat its directory as the project root.
3. Read the configured source directory, defaulting to sources/.
4. Create:

<project-root>/sources/<source-name>/

5. Refuse to run if it cannot find a Lexicon project.

For the Lexicon repository itself, lexicon.toml could explicitly say:

schema_version = 1
[project]
sources_directory = "lexicon-framework/sources"

That preserves its present structure. An external project could use the default:

schema_version = 1
[project]
sources_directory = "sources"

So yes: lexicon init is the true first command for creating a new Lexicon project. lexicon source new creates a source inside an already initialized or cloned Lexicon project. It should not write source projects into the installed framework’s runtime directory.
