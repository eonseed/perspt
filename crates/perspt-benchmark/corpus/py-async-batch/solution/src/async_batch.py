import asyncio

async def map_limited(items, limit, function):
    if limit <= 0: raise ValueError("limit must be positive")
    semaphore = asyncio.Semaphore(limit)
    async def one(item):
        async with semaphore: return await function(item)
    tasks = [asyncio.create_task(one(item)) for item in items]
    try: return await asyncio.gather(*tasks)
    except BaseException:
        for task in tasks: task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        raise
