fn bubble_sort(arr: &mut Vec<i32>) {
    let n = arr.len();
    for i in 0..n {
        let mut swapped = false;
        for j in 0..(n - i - 1) {
            if arr[j] > arr[j + 1] {
                arr.swap(j, j + 1);
                swapped = true;
            }
        }
        if !swapped {
            break; // Early exit if already sorted
        }
    }
}

// Usage example:
fn main() {
    let mut nums = vec![64, 34, 25, 12, 22, 11, 90];
    bubble_sort(&mut nums);
    println!("{:?}", nums); // Output: [11, 12, 22, 25, 34, 64, 90]
}
