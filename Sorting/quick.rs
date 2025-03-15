fn quick_sort(arr: &mut [i32]) {
    if arr.len() < 2 {
        return;
    }
    let pivot = arr[arr.len() - 1];
    let mut partition = 0;
    for i in 0..arr.len() - 1 {
        if arr[i] <= pivot {
            arr.swap(partition, i);
            partition += 1;
        }
    }
    arr.swap(partition, arr.len() - 1);
    quick_sort(&mut arr[0..partition]);
    quick_sort(&mut arr[partition + 1..]);
}

// Usage example:
fn main() {
    let mut nums = vec![64, 34, 25, 12, 22, 11, 90];
    quick_sort(&mut nums);
    println!("{:?}", nums); // Output: [11, 12, 22, 25, 34, 64, 90]
}
