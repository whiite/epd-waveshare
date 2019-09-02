//https://doc.rust-lang.org/book/ch19-06-macros.html?highlight=procedural#how-to-write-a-custom-derive-macro

extern crate proc_macro;

use crate::proc_macro::TokenStream;
use quote::quote;
use syn;

#[proc_macro_derive(Graphics)]
pub fn graphics_derive(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_graphics_macro(&ast)
}


fn impl_graphics_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        use crate::epd2in9::{DEFAULT_BACKGROUND_COLOR, HEIGHT, WIDTH};
        use crate::graphics::{Display, DisplayRotation};
        use crate::prelude::*;
        use embedded_graphics::prelude::*;

        /// Display with Fullsize buffer for use with the 2in9 EPD
        ///
        /// Can also be manuall constructed:
        /// `buffer: [DEFAULT_BACKGROUND_COLOR.get_byte_value(); WIDTH / 8 * HEIGHT]`
        pub struct Display2in9 {
            buffer: [u8; WIDTH as usize * HEIGHT as usize / 8],
            rotation: DisplayRotation,
        }

        impl Default for Display2in9 {
            fn default() -> Self {
                Display2in9 {
                    buffer: [DEFAULT_BACKGROUND_COLOR.get_byte_value();
                        WIDTH as usize * HEIGHT as usize / 8],
                    rotation: DisplayRotation::default(),
                }
            }
        }

        impl Drawing<Color> for Display2in9 {
            fn draw<T>(&mut self, item_pixels: T)
            where
                T: IntoIterator<Item = Pixel<Color>>,
            {
                self.draw_helper(WIDTH, HEIGHT, item_pixels);
            }
        }

        impl Display for Display2in9 {
            fn buffer(&self) -> &[u8] {
                &self.buffer
            }

            fn get_mut_buffer(&mut self) -> &mut [u8] {
                &mut self.buffer
            }

            fn set_rotation(&mut self, rotation: DisplayRotation) {
                self.rotation = rotation;
            }

            fn rotation(&self) -> DisplayRotation {
                self.rotation
            }
        }
        impl HelloMacro for #name {
            fn hello_macro() {
                println!("Hello, Macro! My name is {}", stringify!(#name));
            }
        }
    };
    gen.into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
