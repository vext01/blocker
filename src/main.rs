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
extern crate rustc_mir;
extern crate syntax;

use rustc::session::Session;
use rustc::session::config::{self, Input, ErrorOutputType};
use rustc_driver::{driver, CompilerCalls, Compilation, RustcDefaultCalls};
use syntax::{ast, errors};
use syntax::ast::{NodeId};
use rustc::hir::{Item, TraitItem, ImplItem, Item_};
use rustc_mir::graphviz::write_mir_graphviz;
use rustc::hir::itemlikevisit::ItemLikeVisitor;

use std::path::PathBuf;
use std::process::Command;
use std::fs::File;


struct BlockerHirVisitor {
    func_nodes: Vec<(NodeId, String)>,
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
                self.func_nodes.push((item.id, format!("{:?}", item.name)));
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

            for (id, name) in hir_visitor.func_nodes {
                let did = tcx.hir.local_def_id(id);
                let filename = format!("{}.dot", name);
                let mut file = File::create(filename).expect("failed to create dot file");
                write_mir_graphviz(tcx, vec![did].into_iter(), &mut file).expect(
                    "failed to write graph");
            }
        });
        control
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
