use std::{cmp::Ordering, io};

fn main() {

    let mut guess = String::new();

    io::stdin()
    .read_line(&mut guess)
    .expect("Failed to read line");

    println!("Please Enter your desired number");


    println!("You guessed: {}", guess);

    let secret_number = 5;
    match guess.cmp(&secret_number) {
        Ordering::Less => println!("Too small!"),
        Ordering::Greater => println!("Too big!"),
        Ordering::Equal => println!("You win!"),
    } {
        
    }
}
