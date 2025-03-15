fn shell_sort(arr: &mut Vec<i32>) {
    let mut gap = arr.len() / 2;
    while gap > 0 {
        for i in gap..arr.len() {
            let temp = arr[i];
            let mut j = i;
            while j >= gap && arr[j - gap] > temp {
                arr[j] = arr[j - gap];
                j -= gap;
            }
            arr[j] = temp;
        }
        gap /= 2;
    }
}

// Usage example:
fn main() {
    let mut nums = vec![64, 34, 25, 12, 22, 11, 90];
    shell_sort(&mut nums);
    println!("{:?}", nums); // Output: [11, 12, 22, 25, 34, 64, 90]
}
