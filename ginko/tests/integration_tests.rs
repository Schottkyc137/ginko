use ginko::dts::Project;
use itertools::Itertools;
use std::path::PathBuf;

fn check_no_diagnostics(project: &Project) {
    let diagnostics = project.all_diagnostics().collect_vec();
    if diagnostics.is_empty() {
        return;
    }
    for diagnostic in diagnostics {
        println!("{:?}", diagnostic);
    }
    panic!("Found diagnostics while not expecting any")
}

#[test]
fn no_diagnostics_for_simple_file() {
    let mut project = Project::default();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut file_name = path.clone();
    file_name.push("tests/simple.dts");
    project
        .add_file(file_name.into_os_string().into_string().unwrap())
        .expect("File should be present");
    check_no_diagnostics(&project);
}

#[test]
fn no_diagnostics_for_file_with_delete_node() {
    let mut project = Project::default();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut file_name = path.clone();
    file_name.push("tests/test_delete_syntax_A.dts");
    project
        .add_file(file_name.into_os_string().into_string().unwrap())
        .expect("File should be present");
    let mut file_name = path.clone();
    file_name.push("tests/test_delete_syntax_B.dts");
    project
        .add_file(file_name.into_os_string().into_string().unwrap())
        .expect("File should be present");
    check_no_diagnostics(&project);
}
