use syn::parse_quote;
use syn::spanned::Spanned;

use crate::utils::deref_expr;
use crate::utils::generics_declaration_to_generics;
use crate::utils::signature_to_method_call;
use crate::utils::trait_to_generic_ident;

pub fn derive(trait_: &syn::ItemTrait) -> syn::Result<syn::ItemImpl> {
    // build the methods
    let mut methods: Vec<syn::ImplItemMethod> = Vec::new();
    for item in trait_.items.iter() {
        if let syn::TraitItem::Method(ref m) = item {
            if let Some(receiver) = m.sig.receiver() {
                match receiver {
                    syn::FnArg::Receiver(r) if r.mutability.is_some() => {
                        let msg = "cannot derive `Rc` for a trait declaring `&mut self` methods";
                        return Err(syn::Error::new(r.span(), msg));
                    }
                    syn::FnArg::Receiver(r) if r.reference.is_none() => {
                        let msg = "cannot derive `Rc` for a trait declaring `self` methods";
                        return Err(syn::Error::new(r.span(), msg));
                    }
                    syn::FnArg::Typed(pat) => {
                        let msg = "cannot derive `Rc` for a trait declaring methods with arbitrary receiver types";
                        return Err(syn::Error::new(pat.span(), msg));
                    }
                    _ => (),
                }
            }

            let mut call = signature_to_method_call(&m.sig)?;
            call.receiver = Box::new(deref_expr(deref_expr(*call.receiver)));

            let signature = &m.sig;
            let item = parse_quote!(#[inline] #signature { #call });
            methods.push(item)
        }
    }

    // build an identifier for the generic type used for the implementation
    let trait_ident = &trait_.ident;
    let generic_type = trait_to_generic_ident(&trait_);

    // build the generics for the impl block:
    // we use the same generics as the trait itself, plus
    // a generic type that implements the trait for which we provide the
    // blanket implementation
    let trait_generics = &trait_.generics;
    let where_clause = &trait_.generics.where_clause;
    let mut impl_generics = trait_generics.clone();

    // we must however remove the generic type bounds, to avoid repeating them
    let mut trait_generic_names = trait_generics.clone();
    trait_generic_names.params = generics_declaration_to_generics(&trait_generics.params)?;

    impl_generics.params.push(syn::GenericParam::Type(
        parse_quote!(#generic_type: #trait_ident #trait_generic_names + ?Sized),
    ));

    Ok(parse_quote!(
        #[automatically_derived]
        impl #impl_generics #trait_ident #trait_generic_names for std::rc::Rc<#generic_type> #where_clause {
            #(#methods)*
        }
    ))
}

#[cfg(test)]
mod tests {
    mod derive {

        use syn::parse_quote;

        #[test]
        fn empty() {
            let trait_ = parse_quote!(
                trait Trait {}
            );
            assert_eq!(
                super::super::derive(&trait_).unwrap(),
                parse_quote!(
                    #[automatically_derived]
                    impl<T: Trait + ?Sized> Trait for std::rc::Rc<T> {}
                )
            );
        }

        #[test]
        fn receiver_ref() {
            let trait_ = parse_quote!(
                trait Trait {
                    fn my_method(&self);
                }
            );
            assert_eq!(
                super::super::derive(&trait_).unwrap(),
                parse_quote!(
                    #[automatically_derived]
                    impl<T: Trait + ?Sized> Trait for std::rc::Rc<T> {
                        #[inline]
                        fn my_method(&self) {
                            (*(*self)).my_method()
                        }
                    }
                )
            );
        }

        #[test]
        fn receiver_mut() {
            let trait_ = parse_quote!(
                trait Trait {
                    fn my_method(&mut self);
                }
            );
            assert!(super::super::derive(&trait_).is_err());
        }

        #[test]
        fn receiver_self() {
            let trait_ = parse_quote!(
                trait Trait {
                    fn my_method(self);
                }
            );
            assert!(super::super::derive(&trait_).is_err());
        }

        #[test]
        fn receiver_arbitrary() {
            let trait_ = parse_quote!(
                trait Trait {
                    fn my_method(self: Box<Self>);
                }
            );
            assert!(super::super::derive(&trait_).is_err());
        }

        #[test]
        fn generics() {
            let trait_ = parse_quote!(
                trait MyTrait<T> {}
            );
            let derived = super::super::derive(&trait_).unwrap();

            assert_eq!(
                derived,
                parse_quote!(
                    #[automatically_derived]
                    impl<T, MT: MyTrait<T> + ?Sized> MyTrait<T> for std::rc::Rc<MT> {}
                )
            );
        }

        #[test]
        fn generics_bounded() {
            let trait_ = parse_quote!(
                trait MyTrait<T: 'static + Send> {}
            );
            let derived = super::super::derive(&trait_).unwrap();

            assert_eq!(
                derived,
                parse_quote!(
                    #[automatically_derived]
                    impl<T: 'static + Send, MT: MyTrait<T> + ?Sized> MyTrait<T> for std::rc::Rc<MT> {}
                )
            );
        }

        #[test]
        fn generics_lifetime() {
            let trait_ = parse_quote!(
                trait MyTrait<'a, 'b: 'a, T: 'static + Send> {}
            );
            let derived = super::super::derive(&trait_).unwrap();

            assert_eq!(
                derived,
                parse_quote!(
                    #[automatically_derived]
                    impl<'a, 'b: 'a, T: 'static + Send, MT: MyTrait<'a, 'b, T> + ?Sized>
                        MyTrait<'a, 'b, T> for std::rc::Rc<MT>
                    {
                    }
                )
            );
        }
    }
}
