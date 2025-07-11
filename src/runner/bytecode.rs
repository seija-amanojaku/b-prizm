use crate::nob::*;
use core::ffi::*;
use core::mem::zeroed;
use crate::{printf,strcmp};

#[derive(Clone, Copy, PartialEq)]
pub enum IRBinop {
    Plus,
    Minus,
    Mult,
    Div,
    Mod,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    BitOr,
    BitAnd,
    BitShl,
    BitShr,
}

// We keep this field separate so that it may fail on unsupported versions
const version: u8 = 0x01;
#[derive(Clone, Copy)]
pub struct IRSections {
    pub extern_section: u64,
    pub data_section: u64,
    pub globals_section: u64,
    pub functs_section: u64,
    pub string_table: u64,
}

#[derive(Clone, Copy)]
pub struct IRString {
    pub length: u64,
    pub content: String_Builder
}
#[derive(Clone, Copy)]
pub struct IRExterns {
    pub count: u64,
    pub externs: Array<u64> // Index in String Table
}
#[derive(Clone, Copy)]
pub struct IRData {
    pub length: u64,
    pub data: String_Builder
}

#[derive(Clone, Copy)]
pub enum IRValue {
    Name(u64),
    Literal(u64),
    Offset(u64),
}

// TODO: Parse to a straight Global
#[derive(Clone, Copy)]
pub struct IRGlobal {
    pub name: u64,
    pub values: Array<IRValue>,

    pub is_vec: bool,
    pub min_size: u64
}

#[derive(Clone, Copy)]
pub enum IROpArg {
    Bogus,
    AutoVar(u64),           // ID=0x01
    Deref(u64),             // ID=0x02
    RefExternal(u64),       // ID=0x03
    RefAutoVar(u64),        // ID=0x04
    Literal(u64),           // ID=0x05
    DataOffset(u64),        // ID=0x06
    External(u64),          // ID=0x07
}

#[derive(Clone, Copy)]
pub enum IROpcode {
    Bogus,
    Ret(IROpArg),                               // DONE
    Store(u64, IROpArg),                        // NOT RUNNABLE
    ExtrnSet(IROpArg, IROpArg),                 // NOT RUNNABLE
    AutoSet(u64, IROpArg),                      // DONE
    Negate(u64, IROpArg),                       // DONE
    UnaryNot(u64, IROpArg),                     // DONE
    Binop(u64, IRBinop, IROpArg, IROpArg),      // DONE
    // TODO: Support Asm (code=0x08)
    Label(u64),                                 // DONE
    Jump(u64),                                  // DONE
    JumpNot(u64, IROpArg),                      // DONE

    // Funcall(to, func, args)
    Funcall(IROpArg, IROpArg, Array<IROpArg>)   // NOT STARTED
}

#[derive(Clone, Copy)]
pub struct IRFunction {
    // TODO
    pub name: u64,
    pub file: u64,

    pub params: u64,
    pub autovars: u64,
    pub bodysize: u64,

    // A list from label number to offset in the function bytecode
    pub bytecode: Array<IROpcode>,
    pub labels: Array<usize>
}

#[derive(Clone, Copy)]
pub struct IRInfo {
   pub source: *mut String_Builder,
    pub sections: IRSections,

    pub externs: IRExterns,
    pub data: IRData,
    pub globals: Array<IRGlobal>,
    pub functions: Array<IRFunction>,
    pub string_table: Array<IRString>
}

pub unsafe fn load8(output: *mut String_Builder, offset: usize) -> u8 {
    *((*output).items.add(offset)) as u8
}
pub unsafe fn load64(output: *mut String_Builder, offset: usize) -> u64 {
    let mut val: u64 = 0u64;
    for i in 0..8 {
        // IR integers are little endian
        let byte = 7 - i;
        val <<= 8;
        val |= (*((*output).items.add(byte+offset)) as u8) as u64;
    }

    val
}
pub unsafe fn loadstr(output: *mut String_Builder, offset: usize) -> IRString {
    let mut string: IRString = zeroed();

    string.length = load64(output, offset);
    string.content = zeroed();
    for i in 0u64..string.length {
        let off: usize = i as usize + offset + 8;
        da_append(&mut (string.content), *((*output).items.add(off)));
    }
    da_append(&mut (string.content), 0 as c_char);

    string
}
pub unsafe fn load_externs(ir: *mut IRInfo) {
    let mut offset: usize = (*ir).sections.extern_section as usize;
    let externs: u64 = load64((*ir).source, offset);

    offset += 8;
    for _ in 0u64..externs {
        let index = load64((*ir).source, offset);
        da_append(&mut (*ir).externs.externs, index);
        offset += 8 as usize;
    }
    (*ir).externs.count = externs;
}
pub unsafe fn load_data(ir: *mut IRInfo) {
    let mut offset: usize = (*ir).sections.data_section as usize;
    let count: u64 = load64((*ir).source, offset);

    offset += 8;
    for _ in 0u64..count {
        let byte: c_char = *(*(*ir).source).items.add(offset);
        da_append(&mut (*ir).data.data, byte);
        offset += 1;
    }
    (*ir).data.length = count;
}
pub unsafe fn load_globals(ir: *mut IRInfo) {
    let mut offset: usize = (*ir).sections.globals_section as usize;
    let count: u64 = load64((*ir).source, offset);
    offset += 8;

    for _ in 0u64..count {
        // Try loading a global value
        let name_index = load64((*ir).source, offset);
        offset += 8;
        let count: u64 = load64((*ir).source, offset);
        offset += 8;
        let mut global: IRGlobal = zeroed();

        global.name = name_index;
        global.values = zeroed();
        
        for __ in 0..count {
            let kind: u8 = load8((*ir).source, offset);
            let global_val: IRValue;
            offset += 1;
            if kind == 0x00 {
                let vname = load64((*ir).source, offset);
                global_val = IRValue::Name(vname);
                offset += 8
            } else if kind == 0x01 {
                let literal: u64 = load64((*ir).source, offset);
                offset += 8;
                global_val = IRValue::Literal(literal);
            } else if kind == 0x02 {
                let off: u64 = load64((*ir).source, offset);
                global_val = IRValue::Literal(off);
                offset += 8;
            } else {
                unreachable!("bogus-amogus");
            }
            da_append(&mut global.values, global_val);
        }
        
        let is_vec: bool = load8((*ir).source, offset) != 0;
        offset += 1;
        let min_size: u64 = load64((*ir).source, offset);
        offset += 8;

        global.is_vec = is_vec;
        global.min_size = min_size;
        da_append(&mut (*ir).globals, global);
    }
    (*ir).data.length = count;
}
pub unsafe fn parse_funcarg(ir: *mut IRInfo, offset: *mut usize) -> Option<IROpArg> {
    let arg_byte: u8 = load8((*ir).source, *offset);
    (*offset) += 1;

    return match arg_byte {
        0x00 => {
            let fname = load64((*ir).source, *offset);
            *offset += 8;
            Some(IROpArg::External(fname))
        }
        0x01 => parse_argument(ir, offset),
        _ => unreachable!("bogus-amogus")
    }
}
pub unsafe fn parse_argument(ir: *mut IRInfo, offset: *mut usize) -> Option<IROpArg> {
    let arg_byte: u8 = load8((*ir).source, *offset);
    (*offset) += 1;

    return match arg_byte {
        0x00 => Some(IROpArg::Bogus),
        0x01 => {
            let word = load64((*ir).source, *offset);
            *offset += 8;
            Some(IROpArg::AutoVar(word))
        }
        0x02 => {
            let word = load64((*ir).source, *offset);
            *offset += 8;
            Some(IROpArg::Deref(word))
        }
        0x04 => {
            let word = load64((*ir).source, *offset);
            *offset += 8;
            Some(IROpArg::RefAutoVar(word))
        }
        0x05 => {
            let word = load64((*ir).source, *offset);
            *offset += 8;
            Some(IROpArg::Literal(word))
        }
        0x06 => {
            let word = load64((*ir).source, *offset);
            *offset += 8;
            Some(IROpArg::DataOffset(word))
        }
        0x03 => {
            let irstring = load64((*ir).source, *offset);
            *offset += 8;

            Some(IROpArg::RefExternal(irstring))
        }
        0x07 => {
            /* strings are always prefixed by a size field */
            let irstring = load64((*ir).source, *offset);
            *offset += 8;

            Some(IROpArg::External(irstring))
        }
        _ => unreachable!("bogus-amogus"),
    };
}

pub unsafe fn binop_from_idx(idx: u8) -> Option<IRBinop> {
    return match idx {
        0x00 => Some(IRBinop::Plus         ), 
        0x01 => Some(IRBinop::Minus        ), 
        0x02 => Some(IRBinop::Mod          ),
        0x03 => Some(IRBinop::Div          ), 
        0x04 => Some(IRBinop::Mult         ), 
        0x05 => Some(IRBinop::Less         ), 
        0x06 => Some(IRBinop::Greater      ), 
        0x07 => Some(IRBinop::Equal        ),
        0x08 => Some(IRBinop::NotEqual     ),
        0x09 => Some(IRBinop::GreaterEqual ),
        0x0A => Some(IRBinop::LessEqual    ), 
        0x0B => Some(IRBinop::BitOr        ),
        0x0C => Some(IRBinop::BitAnd       ), 
        0x0D => Some(IRBinop::BitShl       ),
        0x0E => Some(IRBinop::BitShr       ), 
        _    => None
    };
}
pub unsafe fn load_functions(ir: *mut IRInfo) {
    let mut offset: usize = (*ir).sections.functs_section as usize;
    let count: u64 = load64((*ir).source, offset);

    offset += 8;
    for _ in 0u64..count {
        // TODO: Load function
        let mut function: IRFunction = zeroed();

        function.name = load64((*ir).source, offset);
        offset += 8;
        function.file = load64((*ir).source, offset);
        offset += 8;
        function.params = load64((*ir).source, offset);
        offset += 8;
        function.autovars = load64((*ir).source, offset);
        offset += 8;
        function.bodysize = load64((*ir).source, offset);
        offset += 8;

        // TODO: Manage more opcodes
        for __ in 0u64..function.bodysize {
            let _line_number = load64((*ir).source, offset);
            offset += 8;
            let _line_offset = load64((*ir).source, offset);
            offset += 8;
            let opcode_byte: u8 = load8((*ir).source, offset);
            let opcode: IROpcode;
            offset += 1;

            match opcode_byte {
                0x01 => {
                    if let Some(arg) = parse_argument(ir, &mut offset) {
                        opcode = IROpcode::Ret(arg);
                    } else {
                        unreachable!("invalid argument on ret");
                    }
                },
                0x02 => {
                    // Store (index, arg)
                    let idx = load64((*ir).source, offset);
                    offset += 8;
                    let rhs = parse_argument(ir, &mut offset).unwrap();

                    opcode = IROpcode::Store(idx, rhs);
                },
                0x03 => {
                    // ExternSet (str, arg)
                    let lhs = load64((*ir).source, offset);
                    offset += 8;
                    printf(c!("%04X (%lu in string table)\n"), offset as c_int, lhs);
                    let rhs_opt = parse_argument(ir, &mut offset);
                    if let Some(rhs) = rhs_opt {
                        printf(c!("setting %lu\n"), lhs);
                        opcode = IROpcode::ExtrnSet(IROpArg::External(lhs), rhs);
                    } else {
                        unreachable!("invalid argument");
                    }
                },
                0x04 => {
                    // AutoSet (str, arg)
                    let idx = load64((*ir).source, offset);
                    offset += 8;
                    let rhs = parse_argument(ir, &mut offset).unwrap();
                    opcode = IROpcode::AutoSet(idx, rhs);
                },
                0x05 => {
                    // Negate (idx, arg)
                    let idx = load64((*ir).source, offset);
                    offset += 8;
                    let rhs = parse_argument(ir, &mut offset).unwrap();
                    opcode = IROpcode::Negate(idx, rhs);
                },
                0x06 => {
                    // UnaryNot (idx, arg)
                    let idx = load64((*ir).source, offset);
                    offset += 8;
                    let rhs = parse_argument(ir, &mut offset).unwrap();
                    opcode = IROpcode::UnaryNot(idx, rhs);
                },
                0x07 => {
                    // IRBinop (idx, op, arg, arg)
                    // auto[idx] = arg OP arg
                    let to = load64((*ir).source, offset);
                    offset += 8;
                    let binop8 = load8((*ir).source, offset);
                    offset += 1;
                    let arg1 = parse_argument(ir, &mut offset).unwrap();
                    let arg2 = parse_argument(ir, &mut offset).unwrap();
                    let op = binop_from_idx(binop8).unwrap();               // TODO

                    opcode = IROpcode::Binop(to, op, arg1, arg2);
                },
                0x0C => {
                    let to = load64((*ir).source, offset);
                    offset += 8;
                    printf(c!("writing to %d (at 0x%04X)\n"), to as c_uint, offset as c_uint);
                    let func = parse_funcarg(ir, &mut offset).unwrap();
                    let argc = load64((*ir).source, offset);
                    let mut args: Array<IROpArg> = zeroed();
                    offset += 8;

                    printf(c!("%d\n"), argc as c_uint);
                    for _ in 0..argc {
                        let arg = parse_argument(ir, &mut offset).unwrap();
                        da_append(&mut args, arg);
                    }

                    opcode = IROpcode::Funcall(
                        IROpArg::AutoVar(to), 
                        func, args
                    );
                }
                0x09 => {
                    let idx = load64((*ir).source, offset);
                    offset += 8;
                    opcode = IROpcode::Label(idx);

                    // Save our position into the label list
                    // TODO: Should we add 1? Well, it doesn't really matter.
                    da_append(&mut function.labels, function.bytecode.count);
                }
                0x0A => {
                    let idx = load64((*ir).source, offset);
                    offset += 8;
                    opcode = IROpcode::Jump(idx);
                }
                0x0B => {
                    let idx = load64((*ir).source, offset);
                    offset += 8;
                    let arg = parse_argument(ir, &mut offset).unwrap();
                    opcode = IROpcode::JumpNot(idx, arg);
                }
                // TODO: More opcodes!
                _ => unreachable!("invalid opcode :(")
            }
            da_append(&mut function.bytecode, opcode);
        }

        da_append(&mut (*ir).functions, function);
    }
    (*ir).data.length = count;
}

pub unsafe fn load_string_table(ir: *mut IRInfo) {
    let mut offset: usize = (*ir).sections.string_table as usize;
    let count = load64((*ir).source, offset);
    offset += 8;
    for _ in 0..count {
        let str = loadstr((*ir).source, offset);
        da_append(&mut (*ir).string_table, str);
        offset += (8 + str.length) as usize; 
    }
}

pub unsafe fn load_bytecode(ir: *mut IRInfo, output: *mut String_Builder, bytecode_path: *const c_char) -> Option<()> {
    read_entire_file(bytecode_path, output)?;
    let magic: [u8;2] = [ *((*output).items.add(0)) as u8, *((*output).items.add(1)) as u8 ];
    let bvers: u8 = *((*output).items.add(2)) as u8;

    if magic[0] != 0xDE || magic[1] != 0xBC || bvers != version {
        // Invalid/incompatible header
        None
    } else {
        let mut off: usize = (*output).count;

        (*ir).source = output;

        // Start loading the sections
        off -= 8;
        (*ir).sections.string_table = load64(output, off);
        off -= 8;
        (*ir).sections.functs_section = load64(output, off);
        off -= 8;
        (*ir).sections.globals_section = load64(output, off);
        off -= 8;
        (*ir).sections.data_section = load64(output, off);
        off -= 8;
        (*ir).sections.extern_section = load64(output, off);
        
        // Now, start loading each substructure
        load_string_table(ir);
        load_externs(ir);
        load_data(ir);
        load_globals(ir);

        load_functions(ir);
        Some(())
    }
}

pub unsafe fn argument_to_value(_ir: *mut IRInfo, autozone: *mut Array<u64>, arg: IROpArg) -> Option<u64> {
    match arg {
        IROpArg::Bogus => { return Some(0); }
        IROpArg::AutoVar(v) => { return Some(*((*autozone).items.add(v as usize))); }
        IROpArg::Literal(v) => { return Some(v); }
        _ => { return None; }
    };
}

pub unsafe fn get_string(ir: *mut IRInfo, index: u64) -> IRString {
    *(*ir).string_table.items.add(index as usize)
}

pub unsafe fn call_function(ir: *mut IRInfo, name: *const c_char, args: *const Array<u64>) -> u64 {
    // Start by locating that function
    for i in 0..(*ir).functions.count {
        let func: *const IRFunction = (*ir).functions.items.add(i);
        if strcmp(get_string(ir, (*func).name).content.items, name) == 0 {
            // Initialise the autozone
            let mut autozone: Array<u64> = zeroed();
            for j in 0..(*func).autovars {
                let val: u64 = if j < (*func).params {
                    *((*args).items.add(j.try_into().unwrap()))
                } else { 
                    0
                };
                da_append(&mut autozone, val);
            }
            // Now, execute the bytecode
            let mut pc: usize = 0;
            while pc < (*func).bytecode.count {
                let opcode: IROpcode = *((*func).bytecode.items.add(pc));
                match opcode {
                    IROpcode::Ret(arg) => {
                        match arg {
                            IROpArg::Bogus => { return 0; }
                            IROpArg::AutoVar(v) => { return *(autozone.items.add(v as usize)); }
                            IROpArg::Literal(v) => { return v; }
                            _ => todo!("implement argument")
                        }
                    },
                    IROpcode::AutoSet(autovar, val) => {
                        if let Some(v) = argument_to_value(ir, &mut autozone, val) {
                            *(autozone.items.add(autovar as usize)) = v;
                        } else {
                            unreachable!("bad arguments");
                        }
                    },
                    IROpcode::Binop(autovar, op, arg1, arg2) => {
                        let a1 = argument_to_value(ir, &mut autozone, arg1).unwrap();
                        let a2 = argument_to_value(ir, &mut autozone, arg2).unwrap();
                        let result: u64 = match op {
                            IRBinop::Plus           => a1 + a2,
                            IRBinop::Minus          => a1 - a2,
                            IRBinop::BitOr          => a1 | a2,
                            IRBinop::BitAnd         => a1 & a2,
                            IRBinop::BitShl         => a1 << a2,
                            IRBinop::BitShr         => a1 >> a2,
                            IRBinop::Equal          => if a1 == a2 { 1 } else { 0 },
                            IRBinop::NotEqual       => if a1 != a2 { 1 } else { 0 },
                            IRBinop::Mult           => ((a1 as i64) * (a2 as i64)) as u64,
                            IRBinop::Div            => ((a1 as i64) / (a2 as i64)) as u64,
                            IRBinop::Mod            => ((a1 as i64) % (a2 as i64)) as u64,
                            IRBinop::Less           => if (a1 as i64) < (a2 as i64) { 1 } else { 0 },
                            IRBinop::Greater        => if (a1 as i64) > (a2 as i64) { 1 } else { 0 },
                            IRBinop::LessEqual      => if (a1 as i64) <= (a2 as i64) { 1 } else { 0 },
                            IRBinop::GreaterEqual   => if (a1 as i64) >= (a2 as i64) { 1 } else { 0 },
                        };
                        *(autozone.items.add(autovar as usize)) = result;
                    }
                    IROpcode::Label(_idx) => { },               // Labels are as of themselves a no-op
                    IROpcode::Jump(idx) => {
                        // PC is automatically incremented, which skips over the NO-OP label itself.
                        pc = *((*func).labels.items.add(idx as usize)) as usize;
                    },
                    IROpcode::JumpNot(idx, arg1) => {
                        // PC is automatically incremented, which skips over the NO-OP label itself.
                        let arg = argument_to_value(ir, &mut autozone, arg1).unwrap();
                        if arg == 0 {
                            pc = *((*func).labels.items.add(idx as usize)) as usize;
                        }
                    },
                    IROpcode::Negate(idx, arg1) => {
                        let a1 = argument_to_value(ir, &mut autozone, arg1).unwrap();
                        let result: u64 = (-(a1 as i64)) as u64;
                        *(autozone.items.add(idx as usize)) = result;
                    }
                    IROpcode::UnaryNot(idx, arg1) => {
                        let a1 = argument_to_value(ir, &mut autozone, arg1).unwrap();
                        let result: u64 = if a1 == 0 { 1 } else { 0 };
                        *(autozone.items.add(idx as usize)) = result;
                    }
                    _ => unreachable!("unimplemented opcode")
                }
                pc += 1;
            }
            return 0;
        }
    }
    todo!("manage non-existent functions");
}
pub unsafe fn run(_cmd: *mut Cmd, output: *mut String_Builder, bytecode_path: *const c_char, _stdout_path: Option<*const c_char>) -> Option<()> {
    let mut ir: IRInfo = zeroed();
    (*output).count = 0;
    load_bytecode(&mut ir, output, bytecode_path)?;

    // Call that function
    let mut args: Array<u64> = zeroed();
    da_append_many(&mut args, &[ 0, 0 ]);
    let retval = call_function(&mut ir, c!("main"), &args);
    printf(c!("Returned %d...\n"), retval as c_uint);
    todo!("actually implement the bytecode runner");
}


