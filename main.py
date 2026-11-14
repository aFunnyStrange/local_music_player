"""Run the independent desktop player."""
import asyncio
import logging
import sys
from logging.handlers import RotatingFileHandler

import settings
from player.app import main


def configure_logging() -> None:
    """Configure bounded local logs during one-shot startup, including pythonw launches."""
    settings.LOG_FILE.parent.mkdir(parents=True, exist_ok=True)
    handlers = [RotatingFileHandler(settings.LOG_FILE, maxBytes=settings.LOG_MAX_BYTES,
                                   backupCount=settings.LOG_BACKUP_COUNT, encoding="utf-8")]
    if sys.stderr is not None:
        handlers.append(logging.StreamHandler())
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s: %(message)s",
                        handlers=handlers)


if __name__ == "__main__":
    configure_logging()
    asyncio.run(main())

