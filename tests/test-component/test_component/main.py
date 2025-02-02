from datetime import datetime

from pydantic import BaseModel, PositiveInt

from test import exports


class User(BaseModel):
    id: int
    name: str = 'John Doe'
    signup_ts: datetime | None
    tastes: dict[str, PositiveInt]


class Run(exports.Run):
    def run(self) -> None:
        external_data = {
            'id': 123,
            'signup_ts': '2019-06-01 12:22',
            'tastes': {
                'wine': 9,
                b'cheese': 7,
                'cabbage': '1',
            },
        }

        user = User(**external_data)
        print(user.id)
        print(user.model_dump())
