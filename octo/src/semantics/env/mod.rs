use parser::ast::*;
use std::clone::Clone as _;
use std::cell::RefCell;

#[derive(Debug)]
pub struct Function {
    name: String,
    arguments: Vec<Variable>,
    results: Vec<Type>,
}

#[derive(Debug)]
pub struct Variable {
    pub name: String,
    pub span: Span<ByteIndex>,
    pub typ: Type,
    pub used: bool,
}

#[derive(Debug)]
pub struct Scope<'a> {
    //pub functions: Vec<Function>,
    pub variables: RefCell<Vec<Variable>>,
    parent: Option<&'a Scope<'a>>,
}

impl<'a> Scope<'a> {
    pub fn global<'b>() -> Scope<'b> {
        Scope {
            //functions: RefCell::new(vec![]),
            variables: RefCell::new(vec![]),
            parent: None,
        }
    }

    pub fn child_scope(&self) -> Scope {
        Scope {
            //functions: RefCell::new(vec![]),
            variables: RefCell::new(vec![]),
            parent: Some(self),
        }
    }
    pub fn variable_exists(&self, name: &str) -> Option<Span<ByteIndex>> {

        match self.variables.borrow().iter().find(|x| x.name==name) {
            None => {
                match self.parent {
                    None => None,
                    Some(parent) => parent.variable_exists(name),
                }
            },
            Some(x) => {
                Some(x.span)
            }
        }
    }

    pub fn create_variable(&mut self, name: &str, typ: Type, span: Span<ByteIndex>) -> Result<(), Span<ByteIndex>> {
        match self.variable_exists(name) {
            Some(span) => return Result::Err(span),
            None => {},
        };
        self.variables.borrow_mut().push(Variable{
            name: name.to_owned(),
            typ,
            span,
            used: false,
        });
        Result::Ok(())

    }

    pub fn use_variable(&self, name: &str) -> Option<Type> {
        let mut borr = self.variables.borrow_mut();
        let variable = borr.iter_mut().find(|x| x.name == name);
        match variable {
            None => {
                match self.parent {
                    None => None,
                    Some(parent) => {
                        parent.use_variable(name)
                    }
                }
            },
            Some(x) => {
                x.used = true;
                Some(x.typ.clone())

            }
        }

    }

//    pub fn add_function(&mut self, func: &GpuFunction) {
//        self.functions.push(Function{
//            name: func.name.val.to_owned(),
//            arguments: func.arguments.clone(),
//            results: func.results.iter().map(|x| x.typ.clone()).collect(),
//        });
//    }
}
