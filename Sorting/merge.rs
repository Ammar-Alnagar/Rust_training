fn merge_sort(arr: &mut [i32]) {
    if arr.len() < 2 {
        return;
    }
    let mid = arr.len() / 2;
    merge_sort(&mut arr[0..mid]);
    merge_sort(&mut arr[mid..]);
    let mut temp = Vec::with_capacity(arr.len());
    let (mut i, mut j) = (0, mid);
    while i < mid && j < arr.len() {
        if arr[i] <= arr[j] {
            temp.push(arr[i]);
            i += 1;
        } else {
            temp.push(arr[j]);
            j += 1;
        }
    }
    while i < mid {
        temp.push(arr[i]);
        i += 1;
    }
    while j < arr.len() {
        temp.push(arr[j]);
        j += 1;
    }
    arr.copy_from_slice(&temp);
}

// Usage example:
fn main() {
    let mut nums = vec![64, 34, 25, 12, 22, 11, 90];
    merge_sort(&mut nums);
    println!("{:?}", nums); // Output: [11, 12, 22, 25, 34, 64, 90]
}
