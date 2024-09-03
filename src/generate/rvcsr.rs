use anyhow::Result;
use inflections::Inflect;
use proc_macro2::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;

use crate::ir::*;
use crate::util;

use super::sorted;

pub fn render(opts: &super::Options, ir: &IR, b: &Block, path: &str) -> Result<TokenStream> {
    let common_path = opts.common_path();

    let span = Span::call_site();
    let mut items = TokenStream::new();

    for i in sorted(&b.items, |i| (i.byte_offset, i.name.clone())) {
        let name = Ident::new(&i.name, span);
        let offset = util::hex_usize(i.byte_offset as u64);

        let doc = util::doc(&i.description);

        match &i.inner {
            BlockItemInner::Register(r) => {
                let reg_ty = if let Some(fieldset_path) = &r.fieldset {
                    let _f = ir.fieldsets.get(fieldset_path).unwrap();
                    util::relative_path(fieldset_path, path)
                } else {
                    match r.bit_size {
                        8 => quote!(u8),
                        16 => quote!(u16),
                        32 => quote!(u32),
                        64 => quote!(u64),
                        _ => panic!("Invalid register bit size {}", r.bit_size),
                    }
                };

                let access = match r.access {
                    Access::Read => quote!(#common_path::R),
                    Access::Write => quote!(#common_path::W),
                    Access::ReadWrite => quote!(#common_path::RW),
                };

                println!("access -> {:?}", r.access);

                let csr_name = format!("CSR_{}", i.name.to_uppercase());
                let csr_ty = Ident::new(&csr_name, span); // type of CSR: CSR_MTIME

                let ty = quote!(#common_path::Reg<#reg_ty, #csr_ty, #access>);
                if let Some(_array) = &i.array {
                    panic!("register array for csr is not supported!");
                } else {
                    let rasm = format!("csrrs {{0}}, 0x{:03x}, x0", i.byte_offset);
                    let wasm = format!("csrrw x0, 0x{:03x}, {{0}}", i.byte_offset);

                    let csr_trait = quote!(#common_path::CSR);
                    let sealed_csr_trait = quote!(#common_path::SealedCSR);

                    //                   println!("reg_name -> {:?}", reg_name);
                    items.extend(quote!(
                        #doc
                        #[inline(always)]
                        pub const fn #name() -> #ty {
                            unsafe { #common_path::Reg::new() }
                        }

                        #[allow(non_camel_case_types)]
                        #[doc(hidden)]
                        pub struct #csr_ty;

                        impl #sealed_csr_trait for #csr_ty {
                            #[inline]
                            unsafe fn read_csr() -> usize {
                                let r: usize;
                                core::arch::asm!(#rasm, out(reg) r);
                                r
                            }

                            #[inline]
                            unsafe fn write_csr(value: usize) {
                                core::arch::asm!(#wasm, in(reg) value);
                            }
                        }
                        impl #csr_trait for #csr_ty {}
                    ));
                }
            }
            BlockItemInner::Block(b) => {
                panic!("block inside csr is not supported!");
            }
        }
    }

    let (_, name) = super::split_path(path);
    let name = Ident::new(&name.to_lowercase(), span);
    let doc = util::doc(&b.description);
    let out = quote! {
      //  #doc
      //  pub mod #name {
      // output at top level
        #items
      //  }
    };

    Ok(out)
}
