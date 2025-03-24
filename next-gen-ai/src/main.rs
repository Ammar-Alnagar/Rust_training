use std::io::Write;

use kalosm::{*, language::*};

#[tokio::main]
async fn main() {
    let mut llm = Llama::new().await.unwrap();
    let prompt = "The following is a 300 word essay about Paris:";
    print!("{}", prompt);

    let mut stream = llm(prompt);

    stream.to_std_out().await.unwrap();
}