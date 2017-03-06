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

extern crate getopts;
extern crate rustc;
extern crate rustc_driver;
extern crate syntax;

use rustc::session::Session;
use rustc::session::config::{self, Input, ErrorOutputType};
use rustc_driver::{driver, CompilerCalls, Compilation, RustcDefaultCalls};
use syntax::{ast, errors};
use rustc::hir::{Item, TraitItem, ImplItem};
use rustc::hir::itemlikevisit::ItemLikeVisitor;

use std::path::PathBuf;

struct BlockerHirVisitor {}

impl<'a> ItemLikeVisitor<'a> for BlockerHirVisitor {
    fn visit_item(&mut self, item: &Item) {
        // XXX
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
            let krate = tcx.hir.krate(); //expanded_crate.as_ref();

            let mut hir_visitor = BlockerHirVisitor{};
            krate.visit_all_item_likes(&mut hir_visitor);

            // XXX get the DefId out for functions

            // XXX query MIR with DefId:
            // state->tcx->item_mir(DefId)
        });
        control
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    rustc_driver::run_compiler(&args, &mut Blocker::new(), None, None);
}
