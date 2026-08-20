pub mod a;

pub fn api() -> String {
    format!("{}-done", crate::a::helper())
}
