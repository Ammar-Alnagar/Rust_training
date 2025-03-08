use std::{cmp::Ordering, io};

fn main() {
    println!("Guess the number!");
    let secret_number = 5;

    loop {
        println!("Please input your guess.");

    let mut guess = String::new();

    io::stdin()
        .read_line(&mut guess)
        .expect("Failed to read line");

        println!("You guessed: {}", guess.trim());

    let guess: u32 = match guess.trim().parse() {
        Ok(num) => num,
        Err(_) => {
                println!("Invalid input. Please type a number!");
                continue;
        }
    };



    match guess.cmp(&secret_number) {
        Ordering::Less => println!("Too small!"),
        Ordering::Greater => println!("Too big!"),
            Ordering::Equal => {
                println!("You win!");
                break;
    }
}
    }
}
