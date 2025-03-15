fn radix_sort(arr: &mut Vec<u32>) {
    let max_num = *arr.iter().max().unwrap_or(&0);
    let mut exp = 1;
    while max_num / exp > 0 {
        let mut count = vec![0; 10];
        for &num in arr.iter() {
            count[(num / exp % 10) as usize] += 1;
        }
        for i in 1..count.len() {
            count[i] += count[i - 1];
        }
        let mut output = vec![0; arr.len()];
        for &num in arr.iter().rev() {
            let digit = (num / exp % 10) as usize;
            output[count[digit] - 1] = num;
            count[digit] -= 1;
        }
        arr.copy_from_slice(&output);
        exp *= 10;
    }
}

// Usage example:
fn main() {
    let mut nums = vec![170, 45, 75, 90, 802, 24, 2, 66];
    radix_sort(&mut nums);
    println!("{:?}", nums); // Output: [2, 24, 45, 66, 75, 90, 170, 802]
}
