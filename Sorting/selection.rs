fn selection_sort(arr: &mut Vec<i32>) {
    for i in 0..arr.len() {
        let mut min_idx = i;
        for j in (i + 1)..arr.len() {
            if arr[j] < arr[min_idx] {
                min_idx = j;
            }
        }
        arr.swap(i, min_idx);
    }
}

// Usage example:
fn main() {
    let mut nums = vec![64, 34, 25, 12, 22, 11, 90];
    selection_sort(&mut nums);
    println!("{:?}", nums); // Output: [11, 12, 22, 25, 34, 64, 90]
}
