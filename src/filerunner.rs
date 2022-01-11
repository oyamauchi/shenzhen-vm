//! Code to read program input/output from a CSV file, run it, and verify it.

use std::collections::HashMap;
use std::error::Error;
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use crate::components::inputsource::InputSource;
use crate::components::outputsink::OutputSink;
use crate::scheduler::Scheduler;

/// Represents a bus used as input, either a simple I/O pin or an [InputSource].
pub enum InputBus<'a> {
  Simple(&'a Arc<AtomicI32>),
  XBus(&'a InputSource),
}

/// Represents a bus used as output, either a simple I/O pin or an [OutputSink].
pub enum OutputBus<'a> {
  Simple(&'a Arc<AtomicI32>),
  XBus(&'a OutputSink),
}

pub struct FileRunner<'a> {
  reader: BufReader<&'a mut dyn Read>,
  inputs: Vec<(usize, String)>,
  outputs: Vec<(usize, String)>,
}

#[derive(Debug)]
pub struct VerifyError(String);

impl Error for VerifyError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    None
  }
  fn description(&self) -> &str {
    self.0.as_str()
  }

  fn cause(&self) -> Option<&dyn Error> {
    self.source()
  }
}

impl std::fmt::Display for VerifyError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

macro_rules! error {
  ($fmt:literal, $( $arg:expr ),*) => {
    Err(VerifyError(format!($fmt, $( $arg ),*)).into())
  }
}

impl<'a> FileRunner<'a> {
  /// Create a new FileRunner, passing in a [Read] object containing CSV data of inputs and
  /// expected outputs.
  ///
  /// The data should start with a header row. Each field should be of the form `in <name>` or
  /// `out <name>`, indicating whether that field represents an input or an output, and giving it
  /// a name.
  ///
  /// Each data row represents one timestep. For each data row, [FileRunner] will (1) set the
  /// inputs; (2) advance the scheduler; (3) check the outputs. For XBus inputs/outputs of multiple
  /// values per timestep, separate them with spaces. If an input field is blank, that input will
  /// be unchanged in that timestep (simple left as-is, nothing added to XBus). If a simple output
  /// field is blank, it will not be checked in that timestep. If an XBus output field is blank,
  /// FileRunner will check that there was no output on that bus in that timestep.
  ///
  /// NB: this is not parsed as real CSV; in particular, there is no quoting. Since that the only
  /// possible data is integers, there should be no need for quoting.
  pub fn new(in_stream: &'a mut dyn Read) -> Result<FileRunner, std::io::Error> {
    let mut reader = BufReader::new(in_stream);

    let mut header = String::new();
    reader.read_line(&mut header)?;

    let field_specs = header.split(',').map(|s| s.trim());
    let mut inputs = vec![];
    let mut outputs = vec![];

    for (index, field_spec) in field_specs.into_iter().enumerate() {
      if field_spec.starts_with("in ") {
        let name = &field_spec[3..];
        inputs.push((index, String::from(name)));
      } else if field_spec.starts_with("out ") {
        let name = &field_spec[4..];
        outputs.push((index, String::from(name)));
      } else {
        return Err(std::io::Error::new(
          std::io::ErrorKind::InvalidData,
          format!("Invalid field in header: {}", field_spec),
        ));
      }
    }

    Ok(FileRunner {
      reader,
      inputs,
      outputs,
    })
  }

  /// Run the given [Scheduler], verifying actual output against expected.
  ///
  /// The keys in the `inputs` and `outputs` maps must correspond to the CSV headers in the data
  /// file. E.g. for a header `in radio,out display`, `inputs` must have the key `radio`, and
  /// `outputs` must have the key `display`.
  ///
  /// Errors if:
  /// - There are unparseable numbers in the data
  /// - An input/output name in the data is missing from the given HashMaps
  /// - Multiple values are given for a simple input or output
  /// - An output doesn't match
  ///
  /// Returns the number of timesteps verified.
  pub fn verify(
    &mut self,
    scheduler: &mut Scheduler,
    inputs: HashMap<&str, InputBus<'_>>,
    outputs: HashMap<&str, OutputBus<'_>>,
  ) -> Result<usize, Box<dyn Error>> {
    let mut timestep_number = 0;
    let mut buffer = String::new();

    while {
      buffer.clear();
      self
        .reader
        .read_line(&mut buffer)
        .map_or(false, |sz| sz > 0)
    } {
      let split_line: Vec<&str> = buffer.split(',').map(|s| s.trim()).collect();

      for (index, name) in self.inputs.iter() {
        let value_from_file = split_line[*index];
        if value_from_file.len() == 0 {
          continue;
        }

        let values: Vec<&str> = value_from_file.split(' ').collect();

        match inputs.get(name.as_str()) {
          None => {
            return error!("Expected input bus '{}', but not present", name);
          }
          Some(InputBus::Simple(atomic)) => {
            if values.len() == 0 {
              continue;
            } else if values.len() > 1 {
              return error!(
                "Multiple values given for simple input '{}': {:?}",
                name, values
              );
            }
            atomic.store(values[0].parse()?, Ordering::Relaxed)
          }
          Some(InputBus::XBus(source)) => {
            for v in values {
              source.inject(v.parse()?)
            }
          }
        }
      }

      scheduler.advance();
      timestep_number += 1;

      for (index, name) in self.outputs.iter() {
        let value_from_file = split_line[*index];
        let expected: Vec<&str> = if value_from_file.len() > 0 {
          value_from_file.split(' ').collect()
        } else {
          vec![]
        };

        match outputs.get(name.as_str()) {
          None => {
            return error!("Expected output bus '{}', but not present", name);
          }
          Some(OutputBus::Simple(atomic)) => {
            if expected.len() == 0 {
              continue;
            } else if expected.len() > 1 {
              return error!(
                "Multiple values expected for simple output '{}': {:?}",
                name, expected
              );
            }

            let actual = atomic.load(Ordering::Relaxed);
            if expected[0].parse::<i32>()? != actual {
              return error!(
                "Incorrect output '{}' at time {}: expected {}, got {}",
                name, timestep_number, expected[0], actual
              );
            }
          }
          Some(OutputBus::XBus(sink)) => {
            let mut actual = Vec::new();
            sink.queue_into(&mut actual);

            if expected.len() != actual.len() {
              return error!(
                "Incorrect number of values output for '{}' at timestep {}: expected {}, got {}",
                name,
                timestep_number,
                expected.len(),
                actual.len()
              );
            }

            for i in 0..expected.len() {
              if expected[i].parse::<i32>()? != actual[i] {
                return error!(
                  "Incorrect output '{}' at time {}: expected {:?}, got {:?}",
                  name, timestep_number, expected, actual
                );
              }
            }
          }
        };
      }
    }

    Ok(timestep_number)
  }
}
