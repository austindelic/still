use clap::Args;
use mlua::{Lua, Result, Table, Value};
use std::fs;

fn print_value(value: &Value, indent: usize) {
    let pad = "  ".repeat(indent);

    match value {
        Value::Table(table) => {
            for pair in table.clone().pairs::<Value, Value>() {
                let (k, v) = pair.unwrap();
                print!("{pad}{:?}: ", k);
                match v {
                    Value::Table(_) => {
                        println!();
                        print_value(&v, indent + 1);
                    }
                    _ => println!("{:?}", v),
                }
            }
        }
        _ => println!("{pad}{:?}", value),
    }
}

fn main() -> Result<()> {
    let lua = Lua::new();

    let script = fs::read_to_string("formula.lua").expect("failed to read formula.lua");

    let value = lua.load(&script).eval::<Value>()?;

    print_value(&value, 0);

    Ok(())
}
#[derive(Args, Debug, Clone)]
pub struct RunArgs {
    #[arg(value_name = "TASK")]
    pub file_name: String,
}

pub struct RunCommand {
    pub file_name: String,
}

impl From<RunArgs> for RunCommand {
    fn from(args: RunArgs) -> Self {
        Self {
            file_name: args.file_name,
        }
    }
}

impl RunCommand {
    fn print_value(value: &Value, indent: usize) {
        let pad = "  ".repeat(indent);
        match value {
            Value::Table(table) => {
                for pair in table.clone().pairs::<Value, Value>() {
                    let (k, v) = pair.unwrap();
                    print!("{pad}{:?}: ", k);
                    match v {
                        Value::Table(_) => {
                            println!();
                            print_value(&v, indent + 1);
                        }
                        _ => println!("{:?}", v),
                    }
                }
            }
            _ => println!("{pad}{:?}", value),
        }
    }
}

impl RunCommand {
    pub fn run(&self) {
        let lua = Lua::new();
        let script = fs::read_to_string(&self.file_name).expect("failed to read formula.lua");
        let value = lua.load(&script).eval::<Value>().expect("lua failed");
        print_value(&value, 1);
    }
}
