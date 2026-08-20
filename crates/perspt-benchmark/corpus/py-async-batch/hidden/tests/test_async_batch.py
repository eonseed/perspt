import asyncio, pathlib, sys, pytest
sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "src"))
from async_batch import map_limited

def test_order_and_limit():
    async def scenario():
        active = peak = 0
        async def work(value):
            nonlocal active, peak
            active += 1; peak = max(peak, active)
            await asyncio.sleep((4-value) * .001)
            active -= 1
            return value * 2
        assert await map_limited([1,2,3], 2, work) == [2,4,6]
        assert peak <= 2
    asyncio.run(scenario())

def test_boundaries():
    async def scenario():
        assert await map_limited([], 3, lambda x: x) == []
        with pytest.raises(ValueError): await map_limited([1], 0, lambda x: x)
    asyncio.run(scenario())
