
use mirams_proc_macros::generate_recursive_dir_content_list;

// list of pairs (path, content)
// example entry: ("./index.html", "<p>Hello!</p>")
const FILES: &'static [(&'static str, &'static [u8])] = &generate_recursive_dir_content_list!("./dist");

fn main() {
    todo!("unimplemented");
}
