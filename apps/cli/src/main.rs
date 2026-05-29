use ui::cli;
fn add_two(a: u8, b: u8) -> u8 {
    a + b
}
fn main() {
    cli::entry();
    add_two(10, 20);
}
