// Whatever is needed to help with multithreading...

pub enum WorkQueue<T> {
    Work(T),
    Done,
}
