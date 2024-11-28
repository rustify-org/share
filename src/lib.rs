#![deny(clippy::all)]

use std::{collections::HashMap, thread};

use napi::{
  bindgen_prelude::*,
  threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode},
};

use napi::{bindgen_prelude::AsyncTask, Env, JsFunction, JsNumber, Result, Task};


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

struct AsyncFib {
  input: u32,
  with_cache: bool,
}

impl Task for AsyncFib {
  type Output = u32;
  type JsValue = JsNumber;

  fn compute(&mut self) -> Result<Self::Output> {
    // 方法1：无缓存
    // let res = fib(self.input);
    // 方法2:有缓存
    // let mut cache = HashMap::new();
    // let res = fib_with_cache(self.input, &mut cache);
    // Ok(res)
    let res = if self.with_cache {
      fib_with_cache(self.input, &mut HashMap::new())
    } else {
      fib(self.input)
    };
    Ok(res)
  }

  fn resolve(&mut self, env: Env, output: u32) -> Result<Self::JsValue> {
    env.create_uint32(output)
  }
}

pub fn fib(n: u32) -> u32 {
  match n {
    0 | 1 => n,
    _ => fib(n - 1) + fib(n - 2),
  }
}

pub fn fib_with_cache(n: u32, cache: &mut HashMap<u32, u32>) -> u32{
  if let Some(&result) = cache.get(&n) {
    return result;
  }
  let result = match n {
    0|1=>n,
    _=>fib_with_cache(n-1, cache)+fib_with_cache(n-2, cache)
  };
  cache.insert(n, result);
  result
}
// 指定 JS 侧的返回值类型为 Promise<number>
#[napi(ts_return_type="Promise<string>")]
fn async_fib(input: u32, with_cache: bool) -> AsyncTask<AsyncFib> {
  AsyncTask::new(AsyncFib { input, with_cache })
}
// 强制指定参数类型
#[napi(ts_args_type = "callback: (err: null | Error, result: number) => void")]
pub fn call_threadsafe_function(callback: JsFunction) -> Result<()> {
  let tsfn: ThreadsafeFunction<u32, ErrorStrategy::CalleeHandled> = callback
    // ctx.value 即 Rust 调用 JS 函数时传递的入参，封装成 Vec 传递给 JS 函数
    .create_threadsafe_function(0, |ctx| ctx.env.create_uint32(ctx.value).map(|v| vec![v]))?;
  for n in 0..100 {
    let tsfn = tsfn.clone();
    thread::spawn(move || {
      // 通过 tsfn.call 来调用 JS 函数
      tsfn.call(Ok(n), ThreadsafeFunctionCallMode::Blocking);
    });
  }
  Ok(())
}
