# bevy_tasks

A refreshingly simple task executor for bevy. :)

This is a simple threadpool with minimal dependencies. The main usecase is a scoped fork-join, i.e. spawning tasks from
a single thread and having that thread await the completion of those tasks. This is intended specifically for
[`bevy`][bevy] as a lighter alternative to [`rayon`][rayon] for this specific usecase. There are also utilities for
generating the tasks from a slice of data. This library is intended for games and makes no attempt to ensure fairness
or ordering of spawned tasks.

It is based on [`async-executor`][async-executor], a lightweight executor that allows the end user to manage their own threads.
`async-executor` is based on async-task, a core piece of async-std.


一个非常简单的任务执行器.

- 这是一个具有最小依赖关系的简单线程池. 它的主要用途是一个有作用域的 fork-join,即从一个线程,让这个线程等待这些任务的完成.
- 这是专门为[bevy] 作为 [rayon] 的一种更轻的替代品而设计的. 
- 还有一些实用程序用于从数据片生成任务.这个库是为游戏设计的,不会尝试确保生成任务的公平性或顺序.
- 它基于 [async-executor], 这是一个轻量级的执行器,允许最终用户管理自己的线程. [async-executor] 基于 [async-task],这是 [async-std] 的核心部分.
## Usage

In order to be able to optimize task execution in multi-threaded environments,
bevy provides three different thread pools via which tasks of different kinds can be spawned.
(The same API is used in single-threaded environments, even if execution is limited to a single thread.
This currently applies to WASM targets.)
The determining factor for what kind of work should go in each pool is latency requirements:

* For CPU-intensive work (tasks that generally spin until completion) we have a standard
  [`ComputeTaskPool`] and an [`AsyncComputeTaskPool`]. Work that does not need to be completed to
  present the next frame should go to the [`AsyncComputeTaskPool`].

* For IO-intensive work (tasks that spend very little time in a "woken" state) we have an
  [`IoTaskPool`] whose tasks are expected to complete very quickly. Generally speaking, they should just
  await receiving data from somewhere (i.e. disk) and signal other systems when the data is ready
  for consumption. (likely via channels)

  为了能够在多线程环境中优化任务执行,Bevy 提供了三种不同的线程池,通过这些线程池可以生成不同类型的任务(在单线程环境中使用相同的 API,即使执行仅限于单个线程这目前适用于 WASM.)
  决定每个池中应该进行哪种工作的因素是延迟需求:
  
  * 对于 CPU 密集型工作(通常自旋 spin 直到任务完成),我们有一个标准 [ComputeTaskPool] 和 [AsyncComputeTaskPool].
    不需要完成的工作现在下一帧应该转到 ['AsyncComputeTaskPool'].
    
  * 对于 IO 密集型工作(在“唤醒”状态下花费很少时间的任务),我们有一个 [IoTaskPool] 任务被期望非常快完成.
    一般来说,他们应该公正等待从某处(如磁盘)接收数据,并在数据准备好时通知其他系统去消费.(可能通过 channels 渠道).

[bevy]: https://bevyengine.org
[rayon]: https://github.com/rayon-rs/rayon
[async-executor]: https://github.com/stjepang/async-executor



## Dependencies

A very small dependency list is a key feature of this module

```text
├── async-executor
│   ├── async-task
│   ├── concurrent-queue
│   │   └── cache-padded
│   └── fastrand
├── num_cpus
│   └── libc
├── parking
└── futures-lite
```
