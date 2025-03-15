fn heap_sort(arr: &mut Vec<i32>) {
    fn sift_down(arr: &mut [i32], start: usize, end: usize) {
        let mut root = start;
        while root * 2 + 1 <= end {
            let child = root * 2 + 1;
            let mut swap = root;
            if arr[swap] < arr[child] {
                swap = child;
            }
            if child + 1 <= end && arr[swap] < arr[child + 1] {
                swap = child + 1;
            }
            if swap == root {
                break;
            } else {
                arr.swap(root, swap);
                root = swap;
            }
        }
    }

    let n = arr.len();
    for start in (0..n/2).rev() {
        sift_down(arr, start, n-1);
    }
    for end in (1..n).rev() {
        arr.swap(end, 0);
        sift_down(arr, 0, end - 1);
    }
}

// Usage example:
fn main() {
    let mut nums = vec![64, 34, 25, 12, 22, 11, 90];
    heap_sort(&mut nums);
    println!("{:?}", nums); // Output: [11, 12, 22, 25, 34, 64, 90]
}
