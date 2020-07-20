import exogress
import logging
from aiohttp import web

LOGGER_NAME = 'exogress'

logger = logging.getLogger(LOGGER_NAME)

formatter = logging.Formatter('%(asctime)s : %(levelname)s : %(name)s : %(message)s')

terminal = logging.StreamHandler()
terminal.setFormatter(formatter)

logger.addHandler(terminal)

logger.info("serving on 3000")

exogress.spawn(LOGGER_NAME)


async def handle(request):
    return web.Response(text="Hello from exogress on python")


app = web.Application()
app.router.add_get('/', handle)

web.run_app(app, port=3000)
