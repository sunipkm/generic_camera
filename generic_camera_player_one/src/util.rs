/// Discards the first expr and returns the second one, used for being able to repeat an expression
/// as many times as `first`
macro_rules! discard_first {
    ($first:expr, $last:expr) => {
        $last
    };
}
pub(crate) use discard_first;

/// Calls a POA function and converts all `&mut MaybeUninit` out parameters
/// into a `Result<(out...), Error>`. `out_param` are unique names that denote how many parameters
/// after `$args` are originally out parameters so that the macro can know how many arguments to pass.
macro_rules! poa_call {
    ($($func_path:ident)::* ($($args:expr),*$(,)?)) => {{
        let res = $($func_path)::* ($($args,)*);
        res.into_result()
    }};
    ($($func_path:ident)::* ($($args:expr),*$(,)?)  @  $($out_param:ident)*)  => {{
        let ($(mut $out_param,)*) = ($($crate::util::discard_first!($out_param, ::core::mem::MaybeUninit::uninit()),)*);
        let res = $($func_path)::* ($($args,)* $(&mut $out_param),*);
        res.into_result().map(|()| {
            ($($out_param.assume_init()),*)
        })
    }};
}

pub(crate) use poa_call;
