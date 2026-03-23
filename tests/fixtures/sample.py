import os
from typing import Optional


@dataclass
class UserService:
    db: Database

    def get_user(self, user_id: int) -> Optional[dict]:
        """Fetch a user by ID."""
        return self.db.find(user_id)

    def delete_user(self, user_id: int) -> bool:
        return self.db.remove(user_id)


def standalone_function(x: int, y: int) -> int:
    return x + y
