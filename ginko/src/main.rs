use clap::Parser;
use ginko::dts::{Analysis, Diagnostic, DiagnosticPrinter};
use ginko::dts::{FileType, Parser as DtsParser};
use std::error::Error;
use std::fs;

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    file: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let file_name = args.file;
    let content = fs::read_to_string(file_name.clone())?;
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
                return Ok(());
            }
            let mut analysis = Analysis::new(FileType::DtSource);
            analysis.analyze_file(&mut diagnostics, &file);
        }
        Err(err) => {
            let printer = DiagnosticPrinter {
                file_name,
                code: content.lines().map(|line| line.to_string()).collect(),
                diagnostics: &[err],
            };
            println!("{}", printer);
            return Ok(());
        }
    }
    println!("OK; No issues found");
    Ok(())
}
