use std::future::Future;

pub async fn join2<A, B>(a: A, b: B) -> (A::Output, B::Output)
where
    A: Future,
    B: Future,
{
    (a.await, b.await)
}
