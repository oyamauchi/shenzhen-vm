use crate::xbus::TSink;

pub struct OutputSink {
  name: &'static str,
}

impl OutputSink {
  pub fn new(name: &'static str) -> OutputSink {
    OutputSink { name }
  }
}

impl TSink for OutputSink {
  fn write(&self, val: i32) {
    println!("{}: {}", self.name, val)
  }
}
