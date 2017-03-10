// Copyright 2017 King's College London
//   Derived work authored by Edd Barrett <vext01@gmail.com>
//   Original copyright below.
//
// Copyright 2015 Nicholas Cameron.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]
#![feature(rustc_private)]
#[macro_use] extern crate log;


extern crate env_logger;
extern crate getopts;
extern crate rustc;
extern crate rustc_driver;
extern crate syntax;

use rustc::session::Session;
use rustc::session::config::{self, Input, ErrorOutputType};
use rustc_driver::{driver, CompilerCalls, Compilation, RustcDefaultCalls};
use syntax::{ast, errors};
use syntax::ast::{NodeId};
use rustc::hir::{Item, TraitItem, ImplItem, Item_};
use rustc::mir::{Mir, TerminatorKind, BasicBlock};
use rustc::hir::itemlikevisit::ItemLikeVisitor;
use std::io::Write;

use std::path::PathBuf;
use std::process::Command;
use std::fs::File;


struct BlockerHirVisitor {
    func_nodes: Vec<NodeId>,
}

impl BlockerHirVisitor {
    fn new() -> BlockerHirVisitor {
        BlockerHirVisitor {func_nodes: Vec::new()}
    }
}

impl<'a> ItemLikeVisitor<'a> for BlockerHirVisitor {
    fn visit_item(&mut self, item: &Item) {
        match item.node {
            Item_::ItemFn(..) => {
                debug!("found function: {}", item.name);
                self.func_nodes.push(item.id);
            },
            _ => {},
        }
    }

    fn visit_trait_item(&mut self, _: &TraitItem) {
        // unused, but required by trait
    }

    fn visit_impl_item(&mut self, _: &ImplItem) {
        // unused, but required by trait
    }
}

struct Blocker {
    default_calls: RustcDefaultCalls,
}

impl Blocker {
    fn new() -> Blocker {
        Blocker { default_calls: RustcDefaultCalls }
    }
}

impl<'a> CompilerCalls<'a> for Blocker {
    fn early_callback(&mut self,
                      _: &getopts::Matches,
                      _: &config::Options,
                      _: &ast::CrateConfig,
                      _: &errors::registry::Registry,
                      _: ErrorOutputType)
                      -> Compilation {
        Compilation::Continue
    }

    fn late_callback(&mut self,
                     m: &getopts::Matches,
                     s: &Session,
                     i: &Input,
                     odir: &Option<PathBuf>,
                     ofile: &Option<PathBuf>)
                     -> Compilation {
        self.default_calls.late_callback(m, s, i, odir, ofile);
        Compilation::Continue
    }

    fn some_input(&mut self, input: Input, input_path: Option<PathBuf>) -> (Input, Option<PathBuf>) {
        (input, input_path)
    }

    fn no_input(&mut self,
                m: &getopts::Matches,
                o: &config::Options,
                cc: &ast::CrateConfig,
                odir: &Option<PathBuf>,
                ofile: &Option<PathBuf>,
                r: &errors::registry::Registry)
                -> Option<(Input, Option<PathBuf>)> {
        self.default_calls.no_input(m, o, cc, odir, ofile, r)
    }

    fn build_controller(&mut self, _: &Session,  _: &getopts::Matches) -> driver::CompileController<'a> {
        let mut control = driver::CompileController::basic();
        control.after_analysis.stop = Compilation::Continue;
        control.after_analysis.callback = Box::new(|state: &mut driver::CompileState| {
            let tcx = state.tcx.expect("no tcx!");
            let krate = tcx.hir.krate();

            let mut hir_visitor = BlockerHirVisitor::new();
            krate.visit_all_item_likes(&mut hir_visitor);
            debug!("Found {} functions", hir_visitor.func_nodes.len());

            for id in hir_visitor.func_nodes {
                let did = tcx.hir.local_def_id(id);
                let mir_ref = tcx.item_mir(did);
                let mut walker = Walker::new(&mir_ref);
                walker.walk();
            }
        });
        control
    }
}

struct Walker<'a, 'tcx: 'a> {
    mir_ref: &'a Mir<'tcx>,
    dot_file: File,
}

impl<'a, 'tcx: 'a> Walker<'a, 'tcx> {
    fn new(mir_ref: &'a Mir<'tcx>) -> Walker<'a, 'tcx> {
        let mut file = File::create("out.dot").expect("failed to create dot file");
        file.write_all(b"digraph g {\n").expect("write failed");
        Walker{
            mir_ref: mir_ref,
            dot_file: file,
        }

    }

    fn render_node(&mut self, blk_id: &BasicBlock, children: Vec<&BasicBlock>) {
        let label = format!("{:?}\n\nBody", blk_id);
        self.dot_out(&format!("{:?} [label={:?}];\n", blk_id, label));

        for child in children {
            self.dot_out(&format!("{:?} -> {:?};\n", blk_id, child));
        }
    }

    fn walk(&mut self) {
        for (blk_id, blk) in self.mir_ref.basic_blocks().iter_enumerated() {
            println!("block: {:?}", blk_id);
            match blk.terminator {
                None => self.render_node(&blk_id, vec![]),
                Some(ref term) => {
                    match &term.kind {
                        &TerminatorKind::Goto{ref target} =>
                            self.render_node(&blk_id, vec![target]),
                        &TerminatorKind::SwitchInt{ref targets, ..} => {
                            let targets = targets.iter().collect::<Vec<_>>();
                            self.render_node(&blk_id, targets);
                        },
                        &TerminatorKind::Call{ref destination, ref cleanup, ..} => {
                            let mut targets = Vec::new();
                            if let &Some((_, ref t)) = destination {
                                targets.push(t);
                            }
                            if let &Some(ref t) = cleanup {
                                targets.push(t);
                            }
                            self.render_node(&blk_id, targets);
                        },
                        &TerminatorKind::Resume
                            | &TerminatorKind::Return
                            | &TerminatorKind::Unreachable =>
                            self.render_node(&blk_id, vec![]),
                        &TerminatorKind::Drop{target, unwind, ..} => {
                            let mut targets = vec![&target];
                            if let Some(ref t) = unwind {
                                targets.push(t);
                            }
                            self.render_node(&blk_id, targets);
                        },
                        &TerminatorKind::DropAndReplace{target, unwind, ..} => {
                            let mut targets = vec![&target];
                            if let Some(ref t) = unwind {
                                targets.push(t);
                            }
                            self.render_node(&blk_id, targets);
                        },
                        &TerminatorKind::Assert{target, cleanup, ..} => {
                            let mut targets = vec![&target];
                            if let Some(ref t) = cleanup {
                                targets.push(t);
                            }
                            self.render_node(&blk_id, targets);
                        },
                    }
                }
            }
        }

        // terminate dot file
        self.dot_file.write_all(b"}\n").expect("write failed");
    }

    fn dot_out(&mut self, s: &str) {
        self.dot_file.write_all(s.as_bytes()).expect("write failed");
    }
}

/*
 * This program will not work properly without knowing its sysroot, which it is unable to locate
 * itself.
 *
 * Instead of having the user tell us where it is, we instead assume that the sysroot of the rustc
 * in the PATH is the sysroot to use.
 */
fn find_sysroot() -> String {
    let output = Command::new("rustc").arg("--print").arg("sysroot").output()
        .expect("failed to run rustc");
    String::from(String::from_utf8(output.stdout).expect("rustc gave us a weird sysroot").trim())
}

fn main() {
    env_logger::init().unwrap();
    let mut args: Vec<_> = std::env::args().collect();
    args.push(String::from("--sysroot"));
    args.push(find_sysroot());
    rustc_driver::run_compiler(&args, &mut Blocker::new(), None, None);
}
