use clap::Parser;
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use ginko::dts::analysis::project::Project;
use ginko::dts::SeverityMap;
use itertools::Itertools;
use std::error::Error;
use std::path::PathBuf;
use std::process::exit;

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    file: String,
    #[arg(short, long, help = "Add a path to search for include files")]
    include: Option<Vec<String>>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut project = Project::default();

    project.add_include_paths(args.include.unwrap_or_default().iter().map(PathBuf::from));

    let path = PathBuf::from(args.file);

    project.add_file_from_fs(&path)?;
    project.analyze(&path);

    let mut has_errors = false;
    let severities = SeverityMap::default();
    for file in project.project_files() {
        let file = file.borrow();
        let diag = file.diagnostics().cloned().collect_vec();
        if diag.is_empty() {
            continue;
        } else {
            has_errors = true;
        }
        let mut files = SimpleFiles::new();
        let file_id = files.add(
            file.path().unwrap().clone().to_str().unwrap().to_string(),
            file.source(),
        );
        let diagnostics = diag
            .into_iter()
            .map(|diag| diag.into_codespan_diagnostic(file_id, &severities));

        let writer = StandardStream::stderr(ColorChoice::Always);
        let config = codespan_reporting::term::Config::default();

        for diagnostic in diagnostics {
            codespan_reporting::term::emit(&mut writer.lock(), &config, &files, &diagnostic)?;
        }
    }

    if has_errors {
        exit(1);
    } else {
        println!("OK; No issues found");
        exit(0);
    }
}
