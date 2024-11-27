#![deny(clippy::all)]


#[macro_use]
extern crate napi_derive;

#[napi]
pub fn sum(a: i32, b: i32) -> i32 {
  println!("Hello from Rust!");
  return a + b;
}

// 如何调试rust代码
#[napi]
pub fn sub(a: i32, b: i32) -> i32 {
  return a - b;
}

#[napi]
pub fn concat_str(a: String, b: String)->String {
  format!("{}{}", a, b)
}

#[napi(object)]
pub struct ToolOptions {
  pub id: i32,
  pub name: String,
}

#[napi]
pub fn get_options(options: ToolOptions)->ToolOptions {
  println!("id: {}, name: {}", options.id, options.name);
  options
}