mod config;
mod gelf;
mod kafkapool;
mod record;
mod rfc5424;
mod tcpinput;
mod tlsinput;

use self::config::Config;
use self::gelf::Gelf;
use self::kafkapool::KafkaPool;
use self::record::Record;
use self::rfc5424::RFC5424;
use self::tcpinput::TcpInput;
use self::tlsinput::TlsInput;
use std::sync::mpsc::{sync_channel, SyncSender, Receiver};
use std::sync::{Arc, Mutex};

const DEFAULT_INPUT_FORMAT: &'static str = "rfc5424";
const DEFAULT_INPUT_TYPE: &'static str = "syslog-tls";
const DEFAULT_QUEUE_SIZE: usize = 10_000_000;

pub trait Input {
    fn new(config: &Config) -> Self;
    fn accept<TD, TE>(&self, tx: SyncSender<Vec<u8>>, decoder: TD, encoder: TE) where TD: Decoder + Clone + Send + 'static, TE: Encoder + Clone + Send + 'static;
}

pub trait Decoder {
    fn new(config: &Config) -> Self;
    fn decode(&self, line: &str) -> Result<Record, &'static str>;
}

pub trait Encoder {
    fn new(config: &Config) -> Self;
    fn encode(&self, record: Record) -> Result<Vec<u8>, &'static str>;
}

pub trait Output {
    fn new(config: &Config) -> Self;
    fn start(&self, arx: Arc<Mutex<Receiver<Vec<u8>>>>);
}

pub fn start(config_file: &str) {
    let config = match Config::from_path(config_file) {
        Ok(config) => config,
        Err(_) => panic!("Unable to read the config file [{}]", config_file)
    };
    let input_format = config.lookup("input.format").
        map_or(DEFAULT_INPUT_FORMAT, |x| x.as_str().unwrap());
    assert!(input_format == DEFAULT_INPUT_FORMAT);

    let decoder = RFC5424::new(&config);
    let encoder = Gelf::new(&config);
    let output = KafkaPool::new(&config);

    let queue_size = config.lookup("input.queuesize").
        map_or(DEFAULT_QUEUE_SIZE, |x| x.as_integer().unwrap() as usize);

    let (tx, rx): (SyncSender<Vec<u8>>, Receiver<Vec<u8>>) = sync_channel(queue_size);
    let arx = Arc::new(Mutex::new(rx));
    output.start(arx);

    let input_type = config.lookup("input.type").
        map_or(DEFAULT_INPUT_TYPE, |x| x.as_str().unwrap());
    match input_type {
        "syslog-tcp" => TcpInput::new(&config).accept(tx, decoder, encoder),
        "syslog-tls" => TlsInput::new(&config).accept(tx, decoder, encoder),
        _ => panic!("Invalid input type: {}", input_type)
    }
}