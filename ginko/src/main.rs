use clap::Parser;
use ginko::dts::{DiagnosticPrinter, Project, SeverityMap};
use itertools::Itertools;
use std::error::Error;
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

    project.set_include_paths(args.include.unwrap_or_default());
    project.add_file(args.file)?;

    let mut has_errors = false;
    for file in project.project_files() {
        let diag = file.diagnostics().cloned().collect_vec();
        if diag.is_empty() {
            continue;
        } else {
            has_errors = true;
        }
        let code = file.source().lines().map(|it| it.to_owned()).collect_vec();
        let printer = DiagnosticPrinter {
            code,
            diagnostics: &diag,
            severity_map: SeverityMap::default(),
        };
        println!("{}", printer);
    }

    if has_errors {
        println!("OK; No issues found");
        exit(0);
    } else {
        exit(1);
    }
}
