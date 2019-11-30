use std::collections::HashMap;

use super::for_loop::{find_loop, LoopCode};
use super::ids::SpirvIds;
use super::if_else::{find_if_else, IfElseCode};

use super::ir::{Address, Op, Operation, ValueType, StdFunction};
use super::PeekableCode;

use super::spirv;
use super::Builder;
use rspirv::mr::Error as Erro;
use spirv_headers::Word as SpirvAddress;

pub struct MainEmitter<'a, I: std::iter::Iterator<Item = &'a Op>> {
    builder: &'a mut Builder,
    ids: &'a mut SpirvIds,
    value_map: HashMap<Address, SpirvAddress>,
    type_map: HashMap<Address, ValueType>,
    current_block: SpirvAddress,
    last_label: Address,

    input_type: Vec<ValueType>,
    iter: Option<PeekableCode<'a, I>>,
    glsl_id: SpirvAddress,
}

impl<'a, I: std::iter::Iterator<Item = &'a Op>> MainEmitter<'a, I> {
    pub fn new(
        ids: &'a mut SpirvIds,
        module: &'a mut Builder,
        input_type: Vec<ValueType>,
        glsl_inst_id: SpirvAddress,
        iter: I,
    ) -> MainEmitter<'a, I> {
        let map = ids.get_const_mapping();
        let types = ids.get_const_types();
        let current_block = 0;
        // let current_block = module.begin_basic_block(None).unwrap();
        // map.insert(1, current_block);
        Self {
            builder: module,
            ids: ids,
            value_map: map,
            type_map: types,
            current_block,
            last_label: 1,
            input_type,
            iter: Some(PeekableCode::new(iter)),
            glsl_id: glsl_inst_id,
        }
    }

    fn insert(&mut self, old: Address, new: SpirvAddress) {
        self.value_map.insert(old, new);
    }

    fn map(&mut self, address: Address) -> SpirvAddress {
        let mapped = self.try_map(address);
        match mapped {
            None => {
                let id = self.builder.id();
                self.insert(address, id);
                id
            }
            Some(x) => x,
        }
    }

    fn set_type(&mut self, addr: Address, typ: ValueType) {
        let old_type = self.type_map.insert(addr, typ);
        match old_type {
            None => {}
            Some(x) => {
                assert!(x == typ);
            }
        }
    }

    fn get_single_type(&self, addr1: Address) -> ValueType {
        match self.type_map.get(&addr1) {
            None => ValueType::Unknown,
            Some(x) => *x,
        }
    }

    fn get_type(&self, addr1: Address, addr2: Address) -> ValueType {
        let type1 = self.type_map.get(&addr1);
        let type2 = self.type_map.get(&addr2);
        match (type1, type2) {
            (None, None) => ValueType::Unknown,
            (Some(x), None) => *x,
            (None, Some(x)) => *x,
            (Some(x), Some(y)) => {
                // at this point in pipeline this should never happen and means compiler bug
                assert!(*x == *y);
                *x
            }
        }
    }

    fn try_map(&mut self, address: Address) -> Option<SpirvAddress> {
        self.value_map.get(&address).map(|x| *x)
    }
}

impl<'a, I: std::iter::Iterator<Item = &'a Op>> MainEmitter<'a, I> {
    fn emit_arg(&mut self, val_type: ValueType, id: usize, ret: Address) {
        let access = self.ids.sample_arg(id, ret, self.builder);
        self.insert(ret, access);
        self.set_type(ret, val_type);
    }

    fn emit_store(&mut self, addr: Address, ret: Address) {
        let spirv_addr = self.map(addr);
        let ret_addr = self.map(ret);
        let typ = self.get_single_type(addr);
        let ret_type = self.ids.map_type(typ);

        self.builder
            .copy_object(ret_type, Some(ret_addr), spirv_addr).unwrap();
        self.set_type(ret, typ);
    }

    fn emit_construct_vec2(&mut self, addr1: Address, addr2: Address, ret: Address){
        let ret_spirv = self.map(ret);
        let x_spirv = self.map(addr1);
        let y_spirv = self.map(addr2);

        let typ = self.ids.map_type(ValueType::Vec2);

        self.builder.composite_construct(typ, Some(ret_spirv), &[x_spirv, y_spirv]).unwrap();
        self.set_type(ret, ValueType::Vec2);
    }

    fn emit_construct_vec3(&mut self, addr1: Address, addr2: Address, addr3: Address, ret: Address){
        let ret_spirv = self.map(ret);
        let x_spirv = self.map(addr1);
        let y_spirv = self.map(addr2);
        let z_spirv = self.map(addr3);

        let typ = self.ids.map_type(ValueType::Vec3);

        self.builder.composite_construct(typ, Some(ret_spirv), &[x_spirv, y_spirv, z_spirv]).unwrap();
        self.set_type(ret, ValueType::Vec3);
    }

    fn emit_extract(&mut self, vec_addr: Address, id: usize, ret: Address) {
        let ret_spirv = self.map(ret);
        let vec_spirv = self.map(vec_addr);
        let typ = self.ids.map_type(ValueType::Float);

        self.builder.composite_extract(typ, Some(ret_spirv), vec_spirv, &[id as u32]).unwrap();

        self.set_type(ret, ValueType::Float);
    }

    fn emit_algebraic<
        F: Fn(&mut Builder, SpirvAddress, Option<SpirvAddress>, SpirvAddress, SpirvAddress),
        F2: Fn(&mut Builder, SpirvAddress, Option<SpirvAddress>, SpirvAddress, SpirvAddress),
    >(
        &mut self,
        left: Address,
        right: Address,
        ret: Address,
        int_op: F,
        other_op: F2,
    ) {
        let left_address = self.map(left);
        let right_address = self.map(right);
        let typ = self.get_type(left, right);
        let result_address = self.map(ret);

        match typ {
            ValueType::Bool => {
                assert!(false);
            }
            ValueType::Int => {
                let ret_type = self.ids.map_type(ValueType::Int);
                int_op(
                    &mut self.builder,
                    ret_type,
                    Some(result_address),
                    left_address,
                    right_address,
                );
                self.set_type(ret, ValueType::Int);
            }
            typ => {
                // all other types are floats for spir-v
                let ret_type = self.ids.map_type(typ);
                other_op(
                    &mut self.builder,
                    ret_type,
                    Some(result_address),
                    left_address,
                    right_address,
                );
                self.set_type(ret, typ);
            }
        }
    }
    fn emit_add(&mut self, left: Address, right: Address, ret: Address) {
        println!("emit add for l:{} r:{} ret:{}", left, right, ret);
        self.emit_algebraic(
            left,
            right,
            ret,
            |x, a, b, c, d| {
                x.iadd(a, b, c, d).unwrap();
            },
            |x, a, b, c, d| {
                x.fadd(a, b, c, d).unwrap();
            },
        );
    }

    fn emit_sub(&mut self, left: Address, right: Address, ret: Address) {
        self.emit_algebraic(
            left,
            right,
            ret,
            |x, a, b, c, d| {
                x.isub(a, b, c, d).unwrap();
            },
            |x, a, b, c, d| {
                x.fsub(a, b, c, d).unwrap();
            },
        );
    }

    fn emit_mul(&mut self, left: Address, right: Address, ret: Address) {
        self.emit_algebraic(
            left,
            right,
            ret,
            |x, a, b, c, d| {
                x.imul(a, b, c, d).unwrap();
            },
            |x, a, b, c, d| {
                x.fmul(a, b, c, d).unwrap();
            },
        );
    }

    fn emit_div(&mut self, left: Address, right: Address, ret: Address) {
        self.emit_algebraic(
            left,
            right,
            ret,
            |x, a, b, c, d| {
                x.sdiv(a, b, c, d).unwrap();
            },
            |x, a, b, c, d| {
                x.fdiv(a, b, c, d).unwrap();
            },
        );
    }

    pub fn emit_less(&mut self, left: Address, right: Address, ret: Address) {
        let left_address = self.map(left);
        let right_address = self.map(right);
        let typ = self.get_type(left, right);
        let result_address = self.map(ret);

        match typ {
            ValueType::Bool => {
                assert!(false);
            }
            ValueType::Int => {
                let ret_type = self.ids.map_type(ValueType::Bool);
                self.builder
                    .sless_than(ret_type, Some(result_address), left_address, right_address)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Float => {
                // all other types are floats for spir-v
                let ret_type = self.ids.map_type(ValueType::Bool);
                self
                    .builder
                    .ford_less_than(ret_type, Some(result_address), left_address, right_address)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            _ => {
                // we can't compare vectors yet.
                assert!(false);
            }
        }
    }

    pub fn emit_less_eq(&mut self, left: Address, right: Address, ret: Address) {
        let left_address = self.map(left);
        let right_address = self.map(right);
        let typ = self.get_type(left, right);
        let result_address = self.map(ret);

        match typ {
            ValueType::Bool => {
                assert!(false);
            }
            ValueType::Int => {
                let ret_type = self.ids.map_type(ValueType::Bool);
                self.builder
                    .sless_than_equal(ret_type, Some(result_address), left_address, right_address)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Float => {
                let ret_type = self.ids.map_type(ValueType::Bool);
                self.builder
                    .ford_less_than_equal(
                        ret_type,
                        Some(result_address),
                        left_address,
                        right_address,
                    )
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            _ => {
                // we can't compare vectors yet.
                assert!(false);
            }
        }
    }
    pub fn emit_eq(&mut self, left: Address, right: Address, ret: Address) {
        let left_address = self.map(left);
        let right_address = self.map(right);
        let typ = self.get_type(left, right);
        let result_address = self.map(ret);

        let ret_type = self.ids.map_type(ValueType::Bool);

        match typ {
            ValueType::Bool => {
                self.builder
                    .logical_equal(ret_type, Some(result_address), left_address, right_address)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Int => {
                self.builder
                    .iequal(ret_type, Some(result_address), left_address, right_address)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Float => {
                self.builder
                    .ford_equal(ret_type, Some(result_address), left_address, right_address)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Vec2 => {
                let temp_ret_type = self.ids.bool2;
                let ret_addr = self
                    .builder
                    .ford_equal(temp_ret_type, None, left_address, right_address)
                    .unwrap();
                self.builder
                    .all(ret_type, Some(result_address), ret_addr)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Vec3 => {
                let temp_ret_type = self.ids.bool3;
                let ret_addr = self
                    .builder
                    .ford_equal(temp_ret_type, None, left_address, right_address)
                    .unwrap();
                self.builder
                    .all(ret_type, Some(result_address), ret_addr)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Vec4 => {
                let temp_ret_type = self.ids.bool4;
                let ret_addr = self
                    .builder
                    .ford_equal(temp_ret_type, None, left_address, right_address)
                    .unwrap();
                self.builder
                    .all(ret_type, Some(result_address), ret_addr)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            _ => {}
        }
    }
    pub fn emit_neq(&mut self, left: Address, right: Address, ret: Address) {
        let left_address = self.map(left);
        let right_address = self.map(right);
        let typ = self.get_type(left, right);
        let result_address = self.map(ret);

        let ret_type = self.ids.map_type(ValueType::Bool);

        match typ {
            ValueType::Bool => {
                self.builder
                    .logical_not_equal(ret_type, Some(result_address), left_address, right_address)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Int => {
                self.builder
                    .inot_equal(ret_type, Some(result_address), left_address, right_address)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Float => {
                self.builder
                    .ford_not_equal(ret_type, Some(result_address), left_address, right_address)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Vec2 => {
                let temp_ret_type = self.ids.bool2;
                let ret_addr = self
                    .builder
                    .ford_not_equal(temp_ret_type, None, left_address, right_address)
                    .unwrap();
                self.builder
                    .any(ret_type, Some(result_address), ret_addr)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Vec3 => {
                let temp_ret_type = self.ids.bool3;
                let ret_addr = self
                    .builder
                    .ford_not_equal(temp_ret_type, None, left_address, right_address)
                    .unwrap();
                self.builder
                    .any(ret_type, Some(result_address), ret_addr)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            ValueType::Vec4 => {
                let temp_ret_type = self.ids.bool4;
                let ret_addr = self
                    .builder
                    .ford_not_equal(temp_ret_type, None, left_address, right_address)
                    .unwrap();
                self.builder
                    .any(ret_type, Some(result_address), ret_addr)
                    .unwrap();
                self.type_map.insert(ret, ValueType::Bool);
            }
            _ => {}
        }
    }

    fn emit_and(&mut self, left: Address, right: Address, ret: Address) {
        let left_address = self.map(left);
        let right_address = self.map(right);
        let typ = self.get_type(left, right);
        let result_address = self.map(ret);

        let ret_type = self.ids.map_type(ValueType::Bool);

        self.builder
            .logical_and(ret_type, Some(result_address), left_address, right_address)
            .unwrap();
        self.type_map.insert(ret, ValueType::Bool);
    }

    fn emit_or(&mut self, left: Address, right: Address, ret: Address) {
        let left_address = self.map(left);
        let right_address = self.map(right);
        let typ = self.get_type(left, right);
        let result_address = self.map(ret);

        let ret_type = self.ids.map_type(ValueType::Bool);

        self.builder
            .logical_or(ret_type, Some(result_address), left_address, right_address)
            .unwrap();
        self.type_map.insert(ret, ValueType::Bool);
    }

    fn emit_block(&mut self, id: Option<SpirvAddress>) -> SpirvAddress {
        self.current_block = self.builder.begin_basic_block(id).unwrap();
        self.current_block
    }

    fn emit_glsl_ext_instruction(&mut self, id: SpirvAddress, arg: Address, ret: Address) -> SpirvAddress{
        let typ = self.get_single_type(arg);
        let ret_type = self.ids.map_type(typ);
        let spirv_addr = self.map(arg);
        self.type_map.insert(ret, typ);
        let ret = self.map(ret);
        let ret = self.builder.ext_inst(ret_type, Some(ret), self.glsl_id, id, &[spirv_addr]).unwrap();
        ret
    }

    fn emit_selected_glsl(&mut self, int_id: SpirvAddress, float_id: SpirvAddress, arg: Address, ret: Address)-> SpirvAddress {
        let typ = self.get_single_type(arg);
        let ret_type = self.ids.map_type(typ);
        let id = if typ == ValueType::Int {
            int_id
        } else {
            float_id
        };
        let spirv_addr = self.map(arg);
        self.type_map.insert(ret, typ);
        let ret = self.map(ret);
        let ret = self.builder.ext_inst(ret_type, Some(ret), self.glsl_id, id, &[spirv_addr]).unwrap();
        ret

    }
    fn emit_invoke(&mut self, function: StdFunction, ret: Address) -> SpirvAddress {
        use StdFunction as sf;
        let id = match function{
            sf::Round(addr) => self.emit_glsl_ext_instruction(1, addr, ret),
            sf::Trunc(addr)=> self.emit_glsl_ext_instruction(3, addr, ret),
            sf::Abs(addr)=> self.emit_selected_glsl(5, 4, addr, ret),
            sf::Sign(addr)=> self.emit_selected_glsl(7, 6, addr, ret),
            sf::Floor(addr)=> self.emit_glsl_ext_instruction(8, addr, ret),
            sf::Ceil(addr)=> self.emit_glsl_ext_instruction(9, addr, ret),
            sf::Fract(addr)=> self.emit_glsl_ext_instruction(10, addr, ret),
            sf::Radians(addr)=> self.emit_glsl_ext_instruction(11, addr, ret),
            sf::Degrees(addr)=>self.emit_glsl_ext_instruction(12, addr, ret),
            sf::Sin(addr)=> self.emit_glsl_ext_instruction(13, addr, ret),
            sf::Cos(addr)=> self.emit_glsl_ext_instruction(14, addr, ret),
            sf::Tan(addr)=> self.emit_glsl_ext_instruction(15, addr, ret),
            sf::Asin(addr)=> self.emit_glsl_ext_instruction(16, addr, ret),
            sf::Acos(addr)=> self.emit_glsl_ext_instruction(17, addr, ret),
            sf::Atan(addr)=> self.emit_glsl_ext_instruction(18, addr, ret),
            sf::Sinh(addr)=> self.emit_glsl_ext_instruction(19, addr, ret),
            sf::Cosh(addr)=> self.emit_glsl_ext_instruction(20, addr, ret),
            sf::Tanh(addr)=> self.emit_glsl_ext_instruction(21, addr, ret),
            sf::Asinh(addr)=> self.emit_glsl_ext_instruction(22, addr, ret),
            sf::Acosh(addr)=> self.emit_glsl_ext_instruction(23, addr, ret),
            sf::Atanh(addr)=> self.emit_glsl_ext_instruction(24, addr, ret),
            sf::Exp(addr)=>self.emit_glsl_ext_instruction(27, addr, ret),
            sf::Log(addr)=>self.emit_glsl_ext_instruction(28, addr, ret),
            sf::Exp2(addr)=>self.emit_glsl_ext_instruction(29, addr, ret),
            sf::Log2(addr)=>self.emit_glsl_ext_instruction(30, addr, ret),
            sf::Sqrt(addr)=>self.emit_glsl_ext_instruction(31, addr, ret),
            sf::Cross(addr, addr2)=>self.emit_selected_glsl(43, 45, addr, ret),
            sf::Normalize(addr)=>self.emit_glsl_ext_instruction(69, addr, ret),
            sf::Length(addr)=> self.emit_glsl_ext_instruction(66, addr, ret), 

            sf::Clamp(clamped, min, max)=>{
                //self.emit_selected_glsl(43, 45, addr, ret);
                panic!("no impl");
            },
            sf::Min(val1, val2)=>{
                //self.emit_selected_glsl(37, 39, addr, ret);
                panic!("not implemented");
            },
            sf::Max(val1, val2)=>{
                //self.emit_selected_glsl(40, 42, addr, ret);
                panic!("not implemented");
            },
            sf::Atan2(addr, addr2)=> {panic!("not implemented");},
            sf::Pow(addr, addr2)=>{panic!("not implemented");},
        };
        return id;
    }

    fn emit_if_else(&mut self, data: IfElseCode) {
        println!("starting if else");

        let true_label = self.map(data.if_label);
        let end_label = self.map(data.end_label);
        let false_label = if data.else_label.is_some() {
            self.map(data.else_label.unwrap())
        } else {
            end_label
        };

        let cond_address = self.map(data.condition_label);

        let pre_if_block_label = self.current_block;
        self.builder
            .selection_merge(end_label, spirv::SelectionControl::NONE).unwrap();
        self.builder
            .branch_conditional(cond_address, true_label, false_label, &[]).unwrap();
        println!("starting true block");
        self.emit_block(Some(true_label));

        let block_code = data.true_block;
        let mut peekable_code = PeekableCode::new(block_code.iter());

        self.emit_all(&mut peekable_code);

        let post_then_block_label = self.current_block;

        self.builder.branch(end_label).unwrap();

        if data.false_block.is_some() {
            println!("starting false block");
            self.emit_block(Some(false_label));

            let block_code = data.false_block.unwrap();
            let mut peekable_code = PeekableCode::new(block_code.iter());

            self.emit_all(&mut peekable_code);

            self.builder.branch(end_label).unwrap();
        }
        // not used if else block was not emitted
        let post_false_block_label = self.current_block;

        println!(
            "blocks, pre: {}, postif: {}, postelse: {}",
            pre_if_block_label, post_then_block_label, post_false_block_label
        );
        println!(
            "labels, if: {}, else: {:?}, end: {}",
            data.if_label, data.else_label, data.end_label
        );

        self.emit_block(Some(end_label));

        for phi in data.phi_nodes {
            let ret = phi.0;
            let phi_record = match phi.1 {
                Operation::Phi(rec) => rec,
                _ => {
                    panic!("Internal compiler error");
                }
            };

            let typ = if self.type_map.contains_key(&phi_record.new) {
                self.type_map[&phi_record.new]
            } else if self.type_map.contains_key(&phi_record.old) {
                self.type_map[&phi_record.old]
            } else {
                panic!("phi internal compiler error");
            };

            let spirv_type = self.ids.map_type(typ);

            let new_address = self.map(phi_record.new);
            let old_address = self.map(phi_record.old);

            let first = self.map(phi_record.label);
            let second = self.map(phi_record.old_label);

            let ret_addr = self.map(ret);

            self
                .builder
                .phi(
                    spirv_type,
                    Some(ret_addr),
                    &[(new_address, first), (old_address, second)],
                )
                .unwrap();

            self.type_map.insert(ret, typ);
        }
        println!("finished if else");
    }

    fn emit_loop(&mut self, data: LoopCode) {
        let init_label = self.map(data.entry_label);

        let condition_label = self.map(data.condition_label);

        let body_label = self.map(data.body_label);

        let continue_label = self.map(data.continue_label);

        let end_label = self.map(data.exit_label);

        self.builder
            .loop_merge(end_label, continue_label, spirv::LoopControl::NONE, &[])
            .unwrap();
        self.builder.branch(condition_label).unwrap();

        self.emit_block(Some(condition_label));

        let mut peekable_code = PeekableCode::new(data.condition.iter());
        self.emit_all(&mut peekable_code);
        drop(peekable_code);

        let condition = self.map(data.condition_value);

        self.builder
            .branch_conditional(condition, body_label, end_label, &[])
            .unwrap();

        self.emit_block(Some(body_label));
        //emit content
        let mut peekable_code = PeekableCode::new(data.body.iter());
        self.emit_all(&mut peekable_code);
        drop(peekable_code);

        self.builder.branch(continue_label).unwrap();
        self.emit_block(Some(continue_label));

        let mut peekable_code = PeekableCode::new(data.continue_code.iter());
        self.emit_all(&mut peekable_code);
        drop(peekable_code);

        self.builder.branch(init_label).unwrap();

        self.emit_block(Some(end_label));
    }

    pub fn emit(mut self) {
        let mut code = self.iter.take().unwrap();
        self.emit_all(&mut code);
    }

    fn emit_all<'b, I2: std::iter::Iterator<Item = &'b Op>>(
        &mut self,
        code: &mut PeekableCode<'b, I2>,
    ) {
        while let Some((ret, op_code)) = code.next().copied() {
            self.emit_next(ret, op_code, code);
        }
    }

    fn emit_next<'b, I2: std::iter::Iterator<Item = &'b Op>>(
        &mut self,
        ret: usize,
        op_code: Operation,
        code: &mut PeekableCode<'b, I2>,
    ) {
        match op_code {
            Operation::JumpIfElse(..) => {
                let if_data = find_if_else(ret, op_code, code);
                self.emit_if_else(if_data);
            }
            Operation::LoopMerge(..) => {
                let loop_data = find_loop(ret, op_code, code, self.last_label);
                self.emit_loop(loop_data);
            }
            _ => {
                self.emit_operation(ret, op_code);
            }
        }
    }

    fn emit_operation(&mut self, ret: Address, operation: Operation) {
        match operation {
            Operation::Arg(x) => {
                self.emit_arg(self.input_type[x], x, ret);
            }
            Operation::Store(addr) => {
                self.emit_store(addr, ret);
            }
            Operation::ConstructVec2(addr1, addr2) => {
                self.emit_construct_vec2(addr1, addr2, ret);
            }
            Operation::ConstructVec3(addr1, addr2, addr3) => {
                self.emit_construct_vec3(addr1, addr2, addr3, ret);
            }
            Operation::Extract(vec_addr, id) => {
                self.emit_extract(vec_addr, id, ret);
            }
            Operation::Add(left, right) => {
                self.emit_add(left, right, ret);
            }
            Operation::Sub(left, right) => {
                self.emit_sub(left, right, ret);
            }
            Operation::Mul(left, right) => {
                self.emit_mul(left, right, ret);
            }
            Operation::Div(left, right) => {
                self.emit_div(left, right, ret);
            }
            Operation::Neg(..) => {
                // TODO: do
                panic!();
            }
            Operation::Less(left, right) => {
                self.emit_less(left, right, ret);
            }
            Operation::LessEq(left, right) => {
                self.emit_less_eq(left, right, ret);
            }
            Operation::Eq(left, right) => {
                self.emit_eq(left, right, ret);
            }
            Operation::Neq(left, right) => {
                self.emit_neq(left, right, ret);
            }
            Operation::And(left, right) => {
                self.emit_and(left, right, ret);
            }
            Operation::Or(left, right) => {
                self.emit_or(left, right, ret);
            }
            Operation::Label => {
                self.last_label = ret;
                let id = self.map(ret);
                self.emit_block(Some(id));
            }
            Operation::Exit(val, _label) => {
                let value_addr = self.value_map[&val];
                self.ids.store_result(0, value_addr, self.builder);
            }
            Operation::Invoke(function) => {
                self.emit_invoke(function, ret);
            }
            Operation::Phi(rec) => {
                // emit phi
                let phi_record = rec;
                println!("phi for: {:#?}", phi_record);

                let typ = self.get_type(phi_record.new, phi_record.old);
                let spirv_type = self.ids.map_type(typ);
                let new_address = self.map(phi_record.new);
                let old_address = self.map(phi_record.old);
                let ret_address = self.map(ret);

                let first = self.map(phi_record.label);
                let second = self.map(phi_record.old_label);

                self.builder
                    .phi(
                        spirv_type,
                        Some(ret_address),
                        &[(new_address, first), (old_address, second)],
                    )
                    .unwrap();

                self.type_map.insert(ret, typ);
            }
            Operation::StoreInt(..) => (),
            Operation::StoreFloat(..) => (),
            Operation::StoreVec2(..) => (),
            Operation::StoreVec3(..) => (),
            Operation::StoreBool(..) => (),

            Operation::Shift(..) => {
                panic!("internal compiler error: not implemented");
            }
            Operation::Sync(..) => {
                panic!("internal compiler error: should never happen");
            }
            Operation::Jump(label) => {
                let id = self.map(label);
                self.builder.branch(id).unwrap();
            }
            Operation::JumpIfElse(..) | Operation::LoopMerge(..) => {
                panic!("compiler bug. unreachable match arm: {:?}", operation);
            } // _ => (),
        }
    }
}

impl<'a, I: Iterator<Item = &'a Op>> std::ops::Drop for MainEmitter<'a, I> {
    fn drop(&mut self) {
        self.builder.ret().unwrap();
    }
}
