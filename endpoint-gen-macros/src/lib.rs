use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro that implements DefinitionElement for a type that is a valid Definition variant.
///
/// The type must also manually implement GenElement<Self> for this to compile.
/// This enforces that any type used as a Definition variant has proper validation logic.
///
/// # Example
/// ```ignore
/// #[derive(DefinitionVariant)]
/// pub struct EnumElement {
///     config: RustGenConfig,
///     inner: Type,
/// }
///
/// impl GenElement<EnumElement> for EnumElement {
///     fn validate_element(&self) -> eyre::Result<()> {
///         // validation logic
///     }
/// }
/// ```
#[proc_macro_derive(DefinitionVariant)]
pub fn derive_definition_variant(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Generate code that requires GenElement<Self> to be implemented
    // We do this by calling a function that depends on GenElement being in scope
    let expanded = quote! {
        // This impl will only compile if GenElement<#name> is implemented
        impl DefinitionVariant for #name {
            // Empty impl - DefinitionVariant is just a marker trait
        }

        // Compile-time check: ensure GenElement<Self> is implemented
        const _: () = {
            const fn check_gen_element<T: GenElement<T>>() {}
            const fn assert_impl() {
                check_gen_element::<#name>();
            }
        };
    };

    TokenStream::from(expanded)
}
