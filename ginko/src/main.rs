use clap::Parser;
use ginko::dts::{Analysis, Diagnostic, DiagnosticPrinter};
use ginko::dts::{FileType, Parser as DtsParser};
use std::error::Error;
use std::fs;
use std::process::exit;

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    file: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let file_name = args.file;
    let content = fs::read_to_string(file_name.clone())?;
    let file_ending = file_name
        .split('.')
        .last()
        .map(FileType::from_file_ending)
        .unwrap_or_default();
    let mut diagnostics: Vec<Diagnostic> = vec![];

    let mut parser = DtsParser::from_text(content.clone());
    match parser.file() {
        Ok(file) => {
            if !parser.diagnostics.is_empty() {
                let printer = DiagnosticPrinter {
                    file_name,
                    code: content.lines().map(|line| line.to_string()).collect(),
                    diagnostics: &parser.diagnostics,
                };
                println!("{}", printer);
                exit(1);
            }
            let mut analysis = Analysis::new(file_ending);
            analysis.analyze_file(&mut diagnostics, &file);
            if !diagnostics.is_empty() {
                let printer = DiagnosticPrinter {
                    file_name,
                    code: content.lines().map(|line| line.to_string()).collect(),
                    diagnostics: &diagnostics,
                };
                println!("{}", printer);
                exit(1);
            }
        }
        Err(err) => {
            let printer = DiagnosticPrinter {
                file_name,
                code: content.lines().map(|line| line.to_string()).collect(),
                diagnostics: &[err],
            };
            println!("{}", printer);
            exit(1);
        }
    }
    println!("OK; No issues found");
    exit(0);
}
