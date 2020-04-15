#[macro_use]
extern crate pest_derive;

use lazy_static::lazy_static;
use pest::{iterators::Pair, Parser};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

lazy_static! {
    static ref LABELS: Mutex<HashMap<String, usize>> = Mutex::new(HashMap::new());
}

fn main() {
    Compiler::compile("./countdown.asm");
}

#[derive(Parser)]
#[grammar = "../compiler.pest"]
struct Compiler;

impl Compiler {
    fn compile(input: impl AsRef<Path>) {
        let unparsed_file = fs::read_to_string(&input).expect("cannot read file");

        let parsed = Compiler::parse(Rule::file, &unparsed_file)
            .unwrap_or_else(|e| panic!("{}", e))
            .next()
            .unwrap();

        let commands = parsed
            .into_inner()
            .filter(|line| line.as_rule() == Rule::line)
            .enumerate()
            .map(Command::from)
            .collect::<Vec<_>>();

        let out_path = input.as_ref().with_extension("rom");

        let mut out_file = fs::File::create(out_path).unwrap();

        let _ = out_file.write(b"v2.0 raw\n");

        for command in commands.iter() {
            let bits = command.compile();
            let hex = format!("{:04x} ", bits);
            let _ = out_file.write(hex.as_bytes());
        }
    }
}

#[derive(Debug)]
struct Command<'a> {
    index: usize,
    opcode: Opcode,
    mode: Mode,
    operands: [Option<Operand<'a>>; 3],
}

impl<'a> Command<'a> {
    fn compile(&self) -> u16 {
        let mut first_byte = self.opcode.packed();
        let second_byte;

        match self.mode {
            Mode::None => {
                second_byte = 0;
            }
            Mode::Rrr => {
                first_byte +=
                    self.operands[0].unwrap().packed() + self.operands[1].unwrap().unpacked();
                second_byte = self.operands[2].unwrap().unpacked();
            }
            Mode::Rri => {
                first_byte +=
                    self.operands[0].unwrap().packed() + self.operands[1].unwrap().unpacked();

                if self.opcode == Opcode::Beq {
                    let offset =
                        self.operands[2].unwrap().unpacked() as i16 - self.index as i16 - 1;

                    second_byte = offset as u8;
                } else {
                    second_byte = self.operands[2].unwrap().unpacked();
                }
            }
            Mode::Rr => {
                first_byte +=
                    self.operands[0].unwrap().packed() + self.operands[1].unwrap().unpacked();
                second_byte = 0
            }
            Mode::Ri => {
                first_byte += self.operands[0].unwrap().packed();
                second_byte = self.operands[1].unwrap().unpacked();
            }
        }

        ((first_byte as u16) << 8) + second_byte as u16
    }
}

impl<'a> From<(usize, Pair<'a, Rule>)> for Command<'a> {
    fn from(input: (usize, Pair<'a, Rule>)) -> Self {
        let index = input.0;
        let pairs = input.1;

        let mut opcode = None;
        let mut operands = [None; 3];
        let mut operand_idx = 0;

        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::label => {
                    let interior = pair.into_inner().next().unwrap();
                    LABELS
                        .lock()
                        .unwrap()
                        .insert(interior.as_span().as_str().to_owned(), index);
                }
                Rule::opcode => {
                    opcode = Some(Opcode::from(pair));
                }
                Rule::operand => {
                    let operand = Operand::from(pair);
                    operands[operand_idx] = Some(operand);
                    operand_idx += 1;
                }
                _ => {}
            }
        }

        let opcode = opcode.unwrap();
        let mode = opcode.mode();

        Command {
            index,
            opcode,
            mode,
            operands,
        }
    }
}

#[derive(Debug, PartialEq)]
enum Opcode {
    Add,
    Nand,
    Sub,
    Nop,
    Beq,
    Jalr,
    Addi,
    Sw,
    Lw,
    Li,
}

impl Opcode {
    fn mode(&self) -> Mode {
        match self {
            Opcode::Add => Mode::Rrr,
            Opcode::Nand => Mode::Rrr,
            Opcode::Sub => Mode::Rrr,
            Opcode::Nop => Mode::None,
            Opcode::Beq => Mode::Rri,
            Opcode::Jalr => Mode::Rr,
            Opcode::Addi => Mode::Rri,
            Opcode::Sw => Mode::Rri,
            Opcode::Lw => Mode::Rri,
            Opcode::Li => Mode::Ri,
        }
    }
}

impl<'a> From<Pair<'a, Rule>> for Opcode {
    fn from(pairs: Pair<'a, Rule>) -> Self {
        match pairs.as_span().as_str() {
            "add" => Opcode::Add,
            "nand" => Opcode::Nand,
            "sub" => Opcode::Sub,
            "nop" => Opcode::Nop,
            "beq" => Opcode::Beq,
            "jalr" => Opcode::Jalr,
            "addi" => Opcode::Addi,
            "sw" => Opcode::Sw,
            "lw" => Opcode::Lw,
            "li" => Opcode::Li,
            _ => panic!("invalid opcode"),
        }
    }
}
#[derive(Debug)]
enum Mode {
    None,
    Rrr,
    Rri,
    Rr,
    Ri,
}

#[derive(Debug, Clone, Copy)]
enum Operand<'a> {
    Register(Register),
    Immediate(u8),
    Label(&'a str),
}

impl<'a> From<Pair<'a, Rule>> for Operand<'a> {
    fn from(pairs: Pair<'a, Rule>) -> Self {
        let inner = pairs.into_inner().next().unwrap();

        match inner.as_rule() {
            Rule::register => Operand::Register({
                match inner.as_span().as_str() {
                    "rA" => Register::A,
                    "rB" => Register::B,
                    "rC" => Register::C,
                    "rD" => Register::D,
                    _ => panic!("invalid register name"),
                }
            }),
            Rule::number => Operand::Immediate(inner.as_span().as_str().parse::<u8>().unwrap()),
            Rule::label_interior => Operand::Label(inner.as_span().as_str()),
            _ => panic!("unreachable, Operand created from incorrect Rule"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Register {
    A,
    B,
    C,
    D,
}

trait Bits<'a> {
    fn unpacked(&self) -> u8;
    fn bits(&self) -> u8;
    fn packed(&self) -> u8 {
        self.unpacked() << self.bits()
    }
}

impl<'a> Bits<'a> for Opcode {
    fn unpacked(&self) -> u8 {
        match self {
            Opcode::Add => 0,
            Opcode::Nand => 1,
            Opcode::Sub => 2,
            Opcode::Nop => 3,
            Opcode::Beq => 4,
            Opcode::Jalr => 5,
            Opcode::Addi => 6,
            Opcode::Sw => 7,
            Opcode::Lw => 8,
            Opcode::Li => 9,
        }
    }

    fn bits(&self) -> u8 {
        4
    }
}

impl<'a> Bits<'a> for Register {
    fn unpacked(&self) -> u8 {
        match self {
            Register::A => 0,
            Register::B => 1,
            Register::C => 2,
            Register::D => 3,
        }
    }

    fn bits(&self) -> u8 {
        2
    }
}

impl<'a> Bits<'a> for Operand<'a> {
    fn unpacked(&self) -> u8 {
        match self {
            Operand::Register(r) => r.unpacked(),
            Operand::Immediate(i) => *i,
            Operand::Label(label) => *LABELS.lock().unwrap().get(label.to_owned()).unwrap() as u8,
        }
    }

    fn bits(&self) -> u8 {
        2
    }
}
