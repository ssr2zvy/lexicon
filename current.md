The next micro step is not the HTTP call yet. First, make HttpAcquisitionContext know where it must store transactions.

Use this runtime contract:

1. lexicon-framework launches the source executable with:

LEXICON_SOURCE_DIRECTORY=<absolute source directory>

2. run_http_source reads that variable and constructs:

pub struct HttpAcquisitionContext {
    source_directory: PathBuf,
    raw_data_directory: PathBuf,
}

where:

raw_data_directory = <source-directory>/data/raw

3. The generated source implementation receives the populated context without reading the environment variable itself.
4. Reject missing, relative, or invalid source-directory paths with a clear error.

Success criterion: a Core test provides a temporary source directory, constructs the context through the runtime helper, and confirms its raw-data path resolves to:

<temporary-source>/data/raw

After that, implement transaction recording with fake request/response bytes. Add real networking only after the storage behavior works independently.
