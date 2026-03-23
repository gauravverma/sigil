import { Request, Response } from 'express';

interface UserConfig {
    host: string;
    port: number;
}

class UserService {
    private db: Database;

    constructor(db: Database) {
        this.db = db;
    }

    async getUser(id: number): Promise<User | null> {
        return this.db.findById(id);
    }
}

const handler = async (req: Request, res: Response) => {
    res.json({ ok: true });
};

export function validatePort(port: number): boolean {
    return port > 0 && port <= 65535;
}
